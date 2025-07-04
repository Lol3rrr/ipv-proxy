use std::collections::HashMap;

pub struct ConsulConfig {
    pub base_addr: reqwest::Url,
}

impl ConsulConfig {
    pub fn api_url(&self) -> reqwest::Url {
        self.base_addr.join("/v1/").expect("")
    }
}

#[derive(Debug, Clone)]
pub struct ExposedService {
    pub name: String,
    pub public_port: u16,
    pub target_addr: std::net::SocketAddr,
}

pub async fn update_config(client: &reqwest::Client, config: &ConsulConfig) -> Result<Vec<ExposedService>, ()> {
    let services = load_services(&client, config).await?;

    let services_to_expose = services
        .iter()
        .filter_map(|(service, tags)| {
            (tags.iter().any(|v| v == "ipv_proxy.expose")).then_some(service)
        })
        .collect::<Vec<_>>();
    tracing::info!(?services_to_expose, "Services to expose");

    let mut result = Vec::new();
    for service in services_to_expose.iter() {
        let instances = match load_service_nodes(&client, service, config).await {
            Ok(instances) => instances,
            Err(e) => {
                tracing::error!(?e, "Loading Service Nodes");
                continue;
            }
        };

        tracing::debug!(?service, ?instances, "Instances for service");

        for instance in instances.iter() {
            result.push(ExposedService {
                name: instance.service_id.clone(),
                public_port: instance.service_port,
                target_addr: std::net::SocketAddr::new(
                    instance.service_address,
                    instance.service_port,
                ),
            });
        }
    }

    Ok(result)
}

async fn load_services(client: &reqwest::Client, config: &ConsulConfig) -> Result<HashMap<String, Vec<String>>, ()> {
    let target_url = config.api_url().join("catalog/services").unwrap();
    tracing::debug!(?target_url, "Catalog Services URL");

    let response = match client
        .get(target_url)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(?e, "Sending Request");
            return Err(());
        }
    };

    let raw_data = match response.text().await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(?e, "Response");
            return Err(());
        }
    };

    let test = match serde_json::from_str::<HashMap<String, Vec<String>>>(&raw_data) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(?e, "Data");
            return Err(());
        }
    };

    Ok(test)
}

#[derive(Debug, serde::Deserialize)]
pub struct ServiceNode {
    #[serde(rename = "ServiceID")]
    pub service_id: String,
    #[serde(rename = "ServicePort")]
    pub service_port: u16,
    #[serde(rename = "ServiceAddress")]
    pub service_address: std::net::IpAddr,
    #[serde(rename = "ServiceTags")]
    pub service_tags: Vec<String>,
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

async fn load_service_nodes(
    client: &reqwest::Client,
    service: &str,
    config: &ConsulConfig
) -> Result<Vec<ServiceNode>, ()> {
    let target_url = config.api_url().join("catalog/service/").unwrap().join(service).unwrap();
    tracing::debug!(?target_url, "Catalog Service URL");

    let response = match client.get(target_url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(?e, "Sending Request");
            return Err(());
        }
    };

    let raw_data = match response.text().await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(?e, "Response");
            return Err(());
        }
    };

    let test = match serde_json::from_str::<Vec<ServiceNode>>(&raw_data) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(?e, "Data");
            return Err(());
        }
    };

    Ok(test)
}
