pub mod config;
pub mod forward;

pub async fn manage_handlers(consul_config: config::ConsulConfig) {
    use std::collections::{HashMap, HashSet};

    let client = reqwest::Client::new();
    let mut running_handlers = HashMap::<
        String,
        (
            tokio::task::JoinHandle<()>,
            tokio_util::sync::CancellationToken,
        ),
    >::new();

    loop {
        let exposed = match config::update_config(&client, &consul_config).await {
            Ok(exposed) => exposed,
            Err(e) => {
                tracing::error!("Updating Config: {:?}", e);
                tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                continue;
            }
        };

        tracing::debug!(?exposed, "Exposed");

        let exposed_id_names: HashSet<String> =
            exposed.iter().map(|exp| &exp.name).cloned().collect();
        let running_id_names: HashSet<String> = running_handlers.keys().cloned().collect();

        let id_names_to_remove: HashSet<_> =
            running_id_names.difference(&exposed_id_names).collect();
        let id_names_to_start: HashSet<_> =
            exposed_id_names.difference(&running_id_names).collect();

        tracing::debug!(?id_names_to_remove, "Services to stop since last config");
        tracing::debug!(?id_names_to_start, "Services to start since last config");

        for id in id_names_to_remove {
            let (_handle, cancellation) = match running_handlers.remove(id) {
                Some(e) => e,
                None => {
                    tracing::warn!(?id, "No running handler for id");
                    continue;
                }
            };

            cancellation.cancel();

            // TODO
            // Should we abort the handle?
        }

        for id in id_names_to_start {
            let service = exposed.iter().find(|exp| &exp.name == id).unwrap();

            let cancel_token = tokio_util::sync::CancellationToken::new();
            let task_handle = tokio::spawn(forward::forward(service.clone(), cancel_token.clone()));
            let value = (task_handle, cancel_token);

            running_handlers.insert(id.clone(), value);
        }

        running_handlers.retain(|id, (handle, _)| {
            if handle.is_finished() {
                tracing::warn!(?id, "Handler stopped unexpectedly");
                false
            } else {
                true
            }
        });

        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}
