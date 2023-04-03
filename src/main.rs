/*
 * This file is part of Astarte.
 *
 * Copyright 2022 SECO Mind Srl
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 * SPDX-License-Identifier: Apache-2.0
 */
use clap::Parser;
use log::info;

use astarte_device_sdk::options::AstarteOptions;
use astarte_device_sdk::registration;
use astarte_device_sdk::AstarteDeviceSdk;

use astarte_message_hub::config::MessageHubOptions;
use astarte_message_hub::error::AstarteMessageHubError;
use astarte_message_hub::proto_message_hub::message_hub_server::MessageHubServer;
use astarte_message_hub::AstarteHandler;
use astarte_message_hub::AstarteMessageHub;

/// A central service that runs on (Linux) devices for collecting and delivering messages from N
/// apps using 1 MQTT connection to Astarte.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Path to a valid .toml file containing the message hub configuration.
    #[clap(short, long, conflicts_with = "store-directory")]
    toml: Option<String>,
    /// Directory used by Astarte-Message-Hub to retain configuration and other persistent data.
    #[clap(short, long, conflicts_with = "toml")]
    store_directory: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), AstarteMessageHubError> {
    env_logger::init();
    let args = Cli::parse();

    let msg_hub_opts = MessageHubOptions::get(args.toml, args.store_directory).await?;

    // Initailize an Astarte device
    let device_sdk = initialize_astarte_device_sdk(msg_hub_opts).await?;
    info!("Connection to Astarte established.");

    // Create a new Astarte handler
    let handler = AstarteHandler::new(device_sdk);

    // Create a new message hub
    let message_hub = AstarteMessageHub::new(handler.clone());

    // Run the protobuf server
    let addr = "[::1]:50051".parse().unwrap();
    tonic::transport::Server::builder()
        .add_service(MessageHubServer::new(message_hub))
        .serve(addr)
        .await?;

    Ok(())
}

async fn initialize_astarte_device_sdk(
    mut msg_hub_opts: MessageHubOptions,
) -> Result<AstarteDeviceSdk, AstarteMessageHubError> {
    // If no credential secret is present, register a new device using the Astarte device SDK
    if msg_hub_opts.credentials_secret.is_none() {
        msg_hub_opts.credentials_secret = Some(
            registration::register_device(
                msg_hub_opts.pairing_token.as_ref().unwrap(),
                &msg_hub_opts.pairing_url,
                &msg_hub_opts.realm,
                &msg_hub_opts.device_id,
            )
            .await
            .map_err(|err| AstarteMessageHubError::FatalError(err.to_string()))?,
        );
    }
    // Create the configuration options for the device and then instantiate a new device
    let mut device_sdk_opts = AstarteOptions::new(
        &msg_hub_opts.realm,
        &msg_hub_opts.device_id,
        msg_hub_opts.credentials_secret.as_ref().unwrap(),
        &msg_hub_opts.pairing_url,
    );
    if msg_hub_opts.astarte_ignore_ssl {
        device_sdk_opts = device_sdk_opts.ignore_ssl_errors();
    }
    if let Some(int_dir) = &msg_hub_opts.interfaces_directory {
        device_sdk_opts = device_sdk_opts.interface_directory(int_dir)?;
    }
    Ok(AstarteDeviceSdk::new(&device_sdk_opts).await?)
}
