// ! A system for loading a default set of data on startup. This is ONLY enabled in development builds.

use std::{
	io,
	path::{Path, PathBuf},
	time::Duration,
};

use crate::{
	job::JobManagerError,
	library::{LibraryConfig, LibraryManagerError},
	location::{
		delete_location, scan_location, LocationCreateArgs, LocationError, LocationManagerError,
	},
	prisma::location,
};
use futures::{pin_mut, Future, Stream};
use prisma_client_rust::QueryError;
use serde::Deserialize;
use thiserror::Error;
use tokio::{
	fs::{self, metadata},
	time::sleep,
};
use tracing::{info, warn};
use uuid::Uuid;

use crate::library::LibraryManager;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationInitConfig {
	path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInitConfig {
	id: Uuid,
	name: String,
	description: Option<String>,
	#[serde(default)]
	reset_locations_on_startup: bool,
	locations: Vec<LocationInitConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitConfig {
	#[serde(default)]
	reset_on_startup: bool,
	libraries: Vec<LibraryInitConfig>,
	#[serde(skip, default)]
	path: PathBuf,
}

#[derive(Error, Debug)]
pub enum InitConfigError {
	#[error("error loading the init data: {0}")]
	Io(#[from] io::Error),
	#[error("error parsing the init data: {0}")]
	Json(#[from] serde_json::Error),
	#[error("job manager: {0}")]
	JobManager(#[from] JobManagerError),
	#[error("location manager: {0}")]
	LocationManager(#[from] LocationManagerError),
	#[error("library manager: {0}")]
	LibraryManager(#[from] LibraryManagerError),
	#[error("query error: {0}")]
	QueryError(#[from] QueryError),
	#[error("location error: {0}")]
	LocationError(#[from] LocationError),
}

impl InitConfig {
	pub async fn load(data_dir: &Path) -> Result<Option<Self>, InitConfigError> {
		let path = std::env::current_dir()?
			.join(std::env::var("SD_INIT_DATA").unwrap_or("sd_init.json".to_string()));

		if metadata(&path).await.is_ok() {
			let config = fs::read_to_string(&path).await?;
			let mut config: InitConfig = serde_json::from_str(&config)?;
			config.path = path;

			if config.reset_on_startup && data_dir.exists() {
				warn!("previous 'SD_DATA_DIR' was removed on startup!");
				fs::remove_dir_all(&data_dir).await?;
			}

			return Ok(Some(config));
		}

		Ok(None)
	}

	pub async fn apply(self, library_manager: &LibraryManager) -> Result<(), InitConfigError> {
		info!("Initializing app from file: {:?}", self.path);

		for lib in self.libraries {
			let name = lib.name.clone();
			let _guard = AbortOnDrop(tokio::spawn(async move {
				loop {
					info!("Initializing library '{name}' from 'sd_init.json'...");
					sleep(Duration::from_secs(1)).await;
				}
			}));

			let library = match library_manager.get_library(lib.id).await {
				Some(lib) => lib,
				None => {
					let library = library_manager
						.create_with_uuid(
							lib.id,
							LibraryConfig {
								name: lib.name,
								description: lib.description.unwrap_or("".to_string()),
							},
						)
						.await?;

					match library_manager.get_library(library.uuid).await {
						Some(lib) => lib,
						None => {
							warn!(
								"Debug init error: library '{}' was not found after being created!",
								library.config.name
							);
							return Ok(());
						}
					}
				}
			};

			if lib.reset_locations_on_startup {
				let locations = library.db.location().find_many(vec![]).exec().await?;

				for location in locations {
					warn!("deleting location: {:?}", location.path);
					delete_location(&library, location.id).await?;
				}
			}

			for loc in lib.locations {
				if let Some(location) = library
					.db
					.location()
					.find_first(vec![location::path::equals(loc.path.clone())])
					.exec()
					.await?
				{
					warn!("deleting location: {:?}", location.path);
					delete_location(&library, location.id).await?;
				}

				let sd_file = PathBuf::from(&loc.path).join(".spacedrive");
				if sd_file.exists() {
					fs::remove_file(sd_file).await?;
				}

				let location = LocationCreateArgs {
					path: loc.path.clone().into(),
					dry_run: false,
					indexer_rules_ids: Vec::new(),
				}
				.create(&library)
				.await?;
				match location {
					Some(location) => {
						scan_location(&library, location).await?;
					}
					None => {
						warn!(
							"Debug init error: location '{}' was not found after being created!",
							loc.path
						);
					}
				}
			}
		}

		info!("Initialized app from file: {:?}", self.path);
		Ok(())
	}
}

pub struct AbortOnDrop<T>(pub tokio::task::JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
	fn drop(&mut self) {
		self.0.abort()
	}
}

impl<T> Future for AbortOnDrop<T> {
	type Output = Result<T, tokio::task::JoinError>;

	fn poll(
		mut self: std::pin::Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Self::Output> {
		let handle = &mut self.0;

		pin_mut!(handle);

		handle.poll(cx)
	}
}

impl<T> Stream for AbortOnDrop<T> {
	type Item = ();

	fn poll_next(
		mut self: std::pin::Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Option<Self::Item>> {
		let handle = &mut self.0;

		pin_mut!(handle);

		handle.poll(cx).map(|_| None)
	}
}
