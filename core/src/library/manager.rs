use crate::{
	invalidate_query,
	location::LocationManagerError,
	node::Platform,
	object::orphan_remover::OrphanRemoverActor,
	prisma::{location, node, PrismaClient},
	sync::{SyncManager, SyncMessage},
	util::{
		db::{load_and_migrate, MigrationError},
		error::{FileIOError, NonUtf8PathError},
		migrator::MigratorError,
		seeder::{indexer_rules_seeder, SeederError},
	},
	NodeContext,
};

use sd_crypto::{
	keys::keymanager::{KeyManager, StoredKey},
	types::{EncryptedKey, Nonce, Salt},
};

use std::{
	env,
	path::{Path, PathBuf},
	str::FromStr,
	sync::Arc,
};

use thiserror::Error;
use tokio::{fs, io, sync::RwLock, try_join};
use tracing::{debug, error, warn};
use uuid::Uuid;

use super::{Library, LibraryConfig, LibraryConfigWrapped};

/// LibraryManager is a singleton that manages all libraries for a node.
pub struct LibraryManager {
	/// libraries_dir holds the path to the directory where libraries are stored.
	libraries_dir: PathBuf,
	/// libraries holds the list of libraries which are currently loaded into the node.
	libraries: RwLock<Vec<Library>>,
	/// node_context holds the context for the node which this library manager is running on.
	node_context: NodeContext,
}

#[derive(Error, Debug)]
pub enum LibraryManagerError {
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error("error serializing or deserializing the JSON in the config file")]
	Json(#[from] serde_json::Error),
	#[error("database error")]
	Database(#[from] prisma_client_rust::QueryError),
	#[error("library not found error")]
	LibraryNotFound,
	#[error("error migrating the config file")]
	Migration(String),
	#[error("failed to parse uuid")]
	Uuid(#[from] uuid::Error),
	#[error("failed to run seeder")]
	Seeder(#[from] SeederError),
	#[error("failed to initialise the key manager")]
	KeyManager(#[from] sd_crypto::Error),
	#[error("failed to run library migrations")]
	MigratorError(#[from] MigratorError),
	#[error("error migrating the library: {0}")]
	MigrationError(#[from] MigrationError),
	#[error("invalid library configuration: {0}")]
	InvalidConfig(String),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
	#[error("failed to watch locations: {0}")]
	LocationWatcher(#[from] LocationManagerError),
}

impl From<LibraryManagerError> for rspc::Error {
	fn from(error: LibraryManagerError) -> Self {
		rspc::Error::with_cause(
			rspc::ErrorCode::InternalServerError,
			error.to_string(),
			error,
		)
	}
}

pub async fn seed_keymanager(
	client: &PrismaClient,
	km: &Arc<KeyManager>,
) -> Result<(), LibraryManagerError> {
	let mut default = None;

	// collect and serialize the stored keys
	let stored_keys: Vec<StoredKey> = client
		.key()
		.find_many(vec![])
		.exec()
		.await?
		.iter()
		.map(|key| {
			let key = key.clone();
			let uuid = uuid::Uuid::from_str(&key.uuid).expect("invalid key id in the DB");

			if key.default {
				default = Some(uuid);
			}

			Ok(StoredKey {
				uuid,
				version: serde_json::from_str(&key.version)
					.map_err(|_| sd_crypto::Error::Serialization)?,
				key_type: serde_json::from_str(&key.key_type)
					.map_err(|_| sd_crypto::Error::Serialization)?,
				algorithm: serde_json::from_str(&key.algorithm)
					.map_err(|_| sd_crypto::Error::Serialization)?,
				content_salt: Salt::try_from(key.content_salt)?,
				master_key: EncryptedKey::try_from(key.master_key)?,
				master_key_nonce: Nonce::try_from(key.master_key_nonce)?,
				key_nonce: Nonce::try_from(key.key_nonce)?,
				key: key.key,
				hashing_algorithm: serde_json::from_str(&key.hashing_algorithm)
					.map_err(|_| sd_crypto::Error::Serialization)?,
				salt: Salt::try_from(key.salt)?,
				memory_only: false,
				automount: key.automount,
			})
		})
		.collect::<Result<Vec<StoredKey>, sd_crypto::Error>>()?;

	// insert all keys from the DB into the keymanager's keystore
	km.populate_keystore(stored_keys).await?;

	// if any key had an associated default tag
	default.map(|k| km.set_default(k));

	Ok(())
}

impl LibraryManager {
	pub(crate) async fn new(
		libraries_dir: PathBuf,
		node_context: NodeContext,
	) -> Result<Arc<Self>, LibraryManagerError> {
		fs::create_dir_all(&libraries_dir)
			.await
			.map_err(|e| FileIOError::from((&libraries_dir, e)))?;

		let mut libraries = Vec::new();
		let mut read_dir = fs::read_dir(&libraries_dir)
			.await
			.map_err(|e| FileIOError::from((&libraries_dir, e)))?;

		while let Some(entry) = read_dir
			.next_entry()
			.await
			.map_err(|e| FileIOError::from((&libraries_dir, e)))?
		{
			let entry_path = entry.path();
			let metadata = entry
				.metadata()
				.await
				.map_err(|e| FileIOError::from((&entry_path, e)))?;
			if metadata.is_file()
				&& entry_path
					.extension()
					.map(|ext| ext == "sdlibrary")
					.unwrap_or(false)
			{
				let Some(Ok(library_id)) = entry_path
				.file_stem()
				.and_then(|v| v.to_str().map(Uuid::from_str))
			else {
				warn!("Attempted to load library from path '{}' but it has an invalid filename. Skipping...", entry_path.display());
					continue;
			};

				let db_path = entry_path.with_extension("db");
				match fs::metadata(&db_path).await {
					Ok(_) => {}
					Err(e) if e.kind() == io::ErrorKind::NotFound => {
						warn!(
					"Found library '{}' but no matching database file was found. Skipping...",
						entry_path.display()
					);
						continue;
					}
					Err(e) => return Err(FileIOError::from((db_path, e)).into()),
				}

				let config = LibraryConfig::read(entry_path)?;
				libraries
					.push(Self::load(library_id, &db_path, config, node_context.clone()).await?);
			}
		}

		let this = Arc::new(Self {
			libraries: RwLock::new(libraries),
			libraries_dir,
			node_context,
		});

		debug!("LibraryManager initialized");

		Ok(this)
	}

	/// create creates a new library with the given config and mounts it into the running [LibraryManager].
	pub(crate) async fn create(
		&self,
		config: LibraryConfig,
	) -> Result<LibraryConfigWrapped, LibraryManagerError> {
		self.create_with_uuid(Uuid::new_v4(), config).await
	}

	pub(crate) async fn create_with_uuid(
		&self,
		id: Uuid,
		config: LibraryConfig,
	) -> Result<LibraryConfigWrapped, LibraryManagerError> {
		if config.name.is_empty() || config.name.chars().all(|x| x.is_whitespace()) {
			return Err(LibraryManagerError::InvalidConfig(
				"name cannot be empty".to_string(),
			));
		}

		LibraryConfig::save(
			Path::new(&self.libraries_dir).join(format!("{id}.sdlibrary")),
			&config,
		)?;

		let library = Self::load(
			id,
			self.libraries_dir.join(format!("{id}.db")),
			config.clone(),
			self.node_context.clone(),
		)
		.await?;

		// Run seeders
		indexer_rules_seeder(&library.db).await?;

		invalidate_query!(library, "library.list");

		self.libraries.write().await.push(library);
		Ok(LibraryConfigWrapped { uuid: id, config })
	}

	pub(crate) async fn get_all_libraries_config(&self) -> Vec<LibraryConfigWrapped> {
		self.libraries
			.read()
			.await
			.iter()
			.map(|lib| LibraryConfigWrapped {
				config: lib.config.clone(),
				uuid: lib.id,
			})
			.collect()
	}

	// pub(crate) async fn get_all_libraries(&self) -> Vec<Library> {
	// 	self.libraries.read().await.clone()
	// }

	pub(crate) async fn edit(
		&self,
		id: Uuid,
		name: Option<String>,
		description: Option<String>,
	) -> Result<(), LibraryManagerError> {
		// check library is valid
		let mut libraries = self.libraries.write().await;
		let library = libraries
			.iter_mut()
			.find(|lib| lib.id == id)
			.ok_or(LibraryManagerError::LibraryNotFound)?;

		// update the library
		if let Some(name) = name {
			library.config.name = name;
		}
		if let Some(description) = description {
			library.config.description = description;
		}

		LibraryConfig::save(
			Path::new(&self.libraries_dir).join(format!("{id}.sdlibrary")),
			&library.config,
		)?;

		invalidate_query!(library, "library.list");

		for library in self.libraries.read().await.iter() {
			for location in library
				.db
				.location()
				.find_many(vec![])
				.exec()
				.await
				.unwrap_or_else(|e| {
					error!(
						"Failed to get locations from database for location manager: {:#?}",
						e
					);
					vec![]
				}) {
				if let Err(e) = self
					.node_context
					.location_manager
					.add(location.id, library.clone())
					.await
				{
					error!("Failed to add location to location manager: {:#?}", e);
				}
			}
		}

		Ok(())
	}

	pub async fn delete(&self, id: Uuid) -> Result<(), LibraryManagerError> {
		let mut libraries = self.libraries.write().await;

		let library = libraries
			.iter()
			.find(|l| l.id == id)
			.ok_or(LibraryManagerError::LibraryNotFound)?;

		let db_path = self.libraries_dir.join(format!("{}.db", library.id));
		let sd_lib_path = self.libraries_dir.join(format!("{}.sdlibrary", library.id));

		try_join!(
			async {
				fs::remove_file(&db_path)
					.await
					.map_err(|e| LibraryManagerError::FileIO(FileIOError::from((db_path, e))))
			},
			async {
				fs::remove_file(&sd_lib_path)
					.await
					.map_err(|e| LibraryManagerError::FileIO(FileIOError::from((sd_lib_path, e))))
			},
		)?;

		invalidate_query!(library, "library.list");

		libraries.retain(|l| l.id != id);

		Ok(())
	}

	// get_ctx will return the library context for the given library id.
	pub async fn get_library(&self, library_id: Uuid) -> Option<Library> {
		self.libraries
			.read()
			.await
			.iter()
			.find(|lib| lib.id == library_id)
			.map(Clone::clone)
	}

	/// load the library from a given path
	pub(crate) async fn load(
		id: Uuid,
		db_path: impl AsRef<Path>,
		config: LibraryConfig,
		node_context: NodeContext,
	) -> Result<Library, LibraryManagerError> {
		let db_path = db_path.as_ref();
		let db = Arc::new(
			load_and_migrate(&format!(
				"file:{}",
				db_path.as_os_str().to_str().ok_or_else(|| {
					LibraryManagerError::NonUtf8Path(NonUtf8PathError(db_path.into()))
				})?
			))
			.await?,
		);

		let node_config = node_context.config.get().await;

		let platform = match env::consts::OS {
			"windows" => Platform::Windows,
			"macos" => Platform::MacOS,
			"linux" => Platform::Linux,
			_ => Platform::Unknown,
		};

		let uuid_vec = id.as_bytes().to_vec();
		let node_data = db
			.node()
			.upsert(
				node::pub_id::equals(uuid_vec.clone()),
				node::create(
					uuid_vec,
					node_config.name.clone(),
					vec![node::platform::set(platform as i32)],
				),
				vec![node::name::set(node_config.name.clone())],
			)
			.exec()
			.await?;

		let key_manager = Arc::new(KeyManager::new(vec![]).await?);
		seed_keymanager(&db, &key_manager).await?;

		let (sync_manager, mut sync_rx) = SyncManager::new(&db, id);

		tokio::spawn({
			let node_context = node_context.clone();

			async move {
				while let Ok(op) = sync_rx.recv().await {
					let SyncMessage::Created(op) = op else { continue; };

					node_context.p2p.broadcast_sync_events(id, vec![op]).await;
				}
			}
		});

		let library = Library {
			id,
			local_id: node_data.id,
			config,
			key_manager,
			sync: Arc::new(sync_manager),
			orphan_remover: OrphanRemoverActor::spawn(db.clone()),
			db,
			node_local_id: node_data.id,
			node_context,
		};

		for location in library
			.db
			.location()
			.find_many(vec![location::node_id::equals(node_data.id)])
			.exec()
			.await?
		{
			library
				.node_context
				.location_manager
				.add(location.id, library.clone())
				.await?;
		}

		if let Err(e) = library
			.node_context
			.jobs
			.clone()
			.resume_jobs(&library)
			.await
		{
			error!("Failed to resume jobs for library. {:#?}", e);
		}

		Ok(library)
	}
}
