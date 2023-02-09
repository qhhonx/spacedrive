#![cfg_attr(
	all(not(debug_assertions), target_os = "windows"),
	windows_subsystem = "windows"
)]

use std::error::Error;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use sd_core::{custom_uri::create_custom_uri_endpoint, Node};
use tauri::plugin::TauriPlugin;
use tauri::Runtime;
use tauri::{api::path, async_runtime::block_on, Manager, RunEvent};
use tokio::{task::block_in_place, time::sleep};
use tracing::{debug, error};

#[cfg(target_os = "macos")]
mod macos;

mod menu;

#[tauri::command(async)]
async fn app_ready(app_handle: tauri::AppHandle) {
	let window = app_handle.get_window("main").unwrap();

	window.show().unwrap();
}

pub fn spacedrive_plugin_init<R: Runtime>(
	auth_token: &str,
	listen_addr: SocketAddr,
) -> TauriPlugin<R> {
	tauri::plugin::Builder::new("spacedrive")
		.js_init_script(format!(
			r#"window.__SD_CUSTOM_SERVER_AUTH_TOKEN__ = "{auth_token}"; window.__SD_CUSTOM_URI_SERVER__ = "http://{listen_addr}";"#
		))
		.build()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let data_dir = path::data_dir()
		.unwrap_or_else(|| PathBuf::from("./"))
		.join("spacedrive");

	let (node, router) = Node::new(data_dir).await?;

	let app = tauri::Builder::default().plugin(rspc::integrations::tauri::plugin(router, {
		let node = node.clone();
		move || node.get_request_context()
	}));

	// This is a super cringe workaround for: https://github.com/tauri-apps/tauri/issues/3725 & https://bugs.webkit.org/show_bug.cgi?id=146351#c5
	// TODO: Secure this server against other apps on the users machine making requests to it using a HTTP header and random token or something
	let endpoint = create_custom_uri_endpoint(node.clone());
	#[cfg(target_os = "linux")]
	let app = {
		use axum::{
			extract::State,
			http::{Request, StatusCode},
			middleware::{self, Next},
			response::{IntoResponse, Response},
			routing::get,
		};
		use rand::{distributions::Alphanumeric, Rng};
		use std::net::TcpListener;
		use url::Url;

		let signal = server::utils::axum_shutdown_signal(node.clone());

		let auth_token: String = rand::thread_rng()
			.sample_iter(&Alphanumeric)
			.take(10)
			.map(char::from)
			.collect();

		async fn auth_middleware<B>(
			State(auth_token): State<String>,
			request: Request<B>,
			next: Next<B>,
		) -> Response {
			let url = Url::parse(&request.uri().to_string()).unwrap();
			if let Some((_, v)) = url.query_pairs().find(|(k, _)| k == "token") {
				if v == auth_token {
					return next.run(request).await;
				}
			} else if let Some(v) = request
				.headers()
				.get("Authorization")
				.and_then(|v| v.to_str().ok())
			{
				if v == auth_token {
					return next.run(request).await;
				}
			}

			(StatusCode::UNAUTHORIZED, "Unauthorized!").into_response()
		}

		let axum_app = axum::Router::new()
			.route("/", get(|| async { "Spacedrive Server!" }))
			.nest("/spacedrive", endpoint.axum())
			.route_layer(middleware::from_fn_with_state(
				auth_token.clone(),
				auth_middleware,
			))
			.fallback(|| async { "404 Not Found: We're past the event horizon..." });

		let listener = TcpListener::bind("127.0.0.1:0").expect("Error creating localhost server!"); // Only allow current device to access it and randomise port
		let listen_addr = listener
			.local_addr()
			.expect("Error getting localhost server listen addr!");
		debug!("Localhost server listening on: http://{:?}", listen_addr);

		tokio::spawn(async move {
			axum::Server::from_tcp(listener)
				.expect("error creating HTTP server!")
				.serve(axum_app.into_make_service())
				.with_graceful_shutdown(signal)
				.await
				.expect("Error with HTTP server!");
		});

		app.plugin(spacedrive_plugin_init(&auth_token, listen_addr))
	};

	#[cfg(not(target_os = "linux"))]
	let app = app.register_uri_scheme_protocol("spacedrive", endpoint.tauri_uri_scheme("spacedrive"));

	let app = app
		.setup(|app| {
			let app = app.handle();
			app.windows().iter().for_each(|(_, window)| {
				// window.hide().unwrap();

				tokio::spawn({
					let window = window.clone();
					async move {
						sleep(Duration::from_secs(3)).await;
						if !window.is_visible().unwrap_or(true) {
							println!(
							"Window did not emit `app_ready` event fast enough. Showing window..."
						);
							let _ = window.show();
						}
					}
				});

				#[cfg(target_os = "windows")]
				window.set_decorations(true).unwrap();

				#[cfg(target_os = "macos")]
				{
					use macos::*;

					let window = window.ns_window().unwrap();

					unsafe { set_titlebar_style(&window, true, true) };
					unsafe { blur_window_background(&window) };
				}
			});

			Ok(())
		})
		.on_menu_event(menu::handle_menu_event)
		.invoke_handler(tauri::generate_handler![app_ready])
		.menu(menu::get_menu())
		.build(tauri::generate_context!())?;

	app.run(move |app_handler, event| {
		if let RunEvent::ExitRequested { .. } = event {
			debug!("Closing all open windows...");
			app_handler
				.windows()
				.iter()
				.for_each(|(window_name, window)| {
					debug!("closing window: {window_name}");
					if let Err(e) = window.close() {
						error!("failed to close window '{}': {:#?}", window_name, e);
					}
				});

			block_in_place(|| block_on(node.shutdown()));
			app_handler.exit(0);
		}
	});

	Ok(())
}
