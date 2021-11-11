use k8s_openapi::api::core::v1::{
    Container, EnvVar, Pod, PodSpec, ResourceRequirements, Service, ServicePort, ServiceSpec,
    Volume, VolumeMount,
};
use kube::api::ObjectMeta;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::utils::{
    get_dataloader_pod_name, get_emb_server_pod_name, get_metrics_gateway_pod_name,
    get_metrics_gateway_service_name, get_mid_server_pod_name, get_trainer_pod_name,
    DEFAULT_CUDA_IMAGE,
};

#[derive(CustomResource, Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[kube(
    group = "persia.com",
    version = "v1",
    kind = "PersiaJob",
    plural = "persiajobs",
    derive = "PartialEq",
    namespaced
)]
#[allow(non_snake_case)]
pub struct PersiaJobSpec {
    pub globalConfigPath: String,
    pub embeddingConfigPath: String,
    pub trainerPyEntryPath: Option<String>,
    pub dataLoaderPyEntryPath: Option<String>,
    pub enableMetrics: Option<bool>,
    pub volumes: Option<Vec<Volume>>,
    pub env: Option<Vec<EnvVar>>,
    pub logLevel: Option<String>,
    pub embeddingServer: Option<EmbeddingSpec>,
    pub middlewareServer: Option<MiddlewareSpec>,
    pub trainer: Option<TrainerSpec>,
    pub dataloader: Option<DataLoaderSpec>,
}

impl PersiaJobSpec {
    fn gen_pod_template(&self, job_name: &str, namespace: &str) -> Pod {
        let log_level = self.logLevel.clone().unwrap_or(String::from("info"));

        let mut labels: BTreeMap<String, String> = BTreeMap::new();
        labels.insert("app".to_owned(), job_name.to_owned());

        let mut env = vec![
            EnvVar {
                name: String::from("EMBEDDING_CONFIG_PATH"),
                value: Some(self.embeddingConfigPath.clone()),
                ..EnvVar::default()
            },
            EnvVar {
                name: String::from("GLOBAL_CONFIG_PATH"),
                value: Some(self.globalConfigPath.clone()),
                ..EnvVar::default()
            },
            EnvVar {
                name: String::from("LOG_LEVEL"),
                value: Some(log_level),
                ..EnvVar::default()
            },
            EnvVar {
                name: String::from("RUST_BACKTRACE"),
                value: Some(String::from("full")),
                ..EnvVar::default()
            },
            EnvVar {
                name: String::from("PERSIA_METRICS_GATEWAY_ADDR"),
                value: Some(format!(
                    "{}:9091",
                    get_metrics_gateway_service_name(job_name)
                )),
                ..EnvVar::default()
            },
        ];

        if let Some(e) = &self.env {
            e.iter().for_each(|env_var| {
                env.push(env_var.clone());
            });
        }

        let pod_spec = PodSpec {
            containers: vec![Container {
                command: Some(vec!["persia_launcher".to_string()]),
                env: Some(env),
                image_pull_policy: Some(String::from("IfNotPresent")),
                ..Container::default()
            }],
            volumes: self.volumes.clone(),
            restart_policy: Some(String::from("Never")),
            ..PodSpec::default()
        };

        Pod {
            metadata: ObjectMeta {
                namespace: Some(namespace.to_owned()),
                labels: Some(labels.clone()),
                ..ObjectMeta::default()
            },
            spec: Some(pod_spec),
            ..Pod::default()
        }
    }

    pub fn gen_services(&self, job_name: &str, namespace: &str) -> Vec<Service> {
        let mut results = Vec::new();

        let mut labels: BTreeMap<String, String> = BTreeMap::new();
        labels.insert("app".to_owned(), job_name.to_owned());

        if self.enableMetrics.unwrap_or(true) {
            let service_name = get_metrics_gateway_service_name(job_name);

            let mut selector: BTreeMap<String, String> = BTreeMap::new();
            selector.insert("service".to_owned(), service_name.clone());

            let metrics_gateway_service = Service {
                metadata: ObjectMeta {
                    name: Some(service_name.clone()),
                    namespace: Some(namespace.to_owned()),
                    labels: Some(labels.clone()),
                    ..ObjectMeta::default()
                },
                spec: Some(ServiceSpec {
                    ports: Some(vec![ServicePort {
                        port: 9091,
                        ..ServicePort::default()
                    }]),
                    selector: Some(selector),
                    ..ServiceSpec::default()
                }),
                ..Service::default()
            };

            results.push(metrics_gateway_service);
        }

        results
    }

    pub fn gen_pods(&self, job_name: &str, namespace: &str) -> Vec<Pod> {
        let mut results = Vec::new();

        if let Some(embedding_server) = &self.embeddingServer {
            let mut emb_server_spec: Vec<Pod> = (0..embedding_server.replicas)
                .into_iter()
                .map(|replica_idx| {
                    let mut pod = self.gen_pod_template(job_name, namespace);

                    let podspec = pod.spec.as_mut().unwrap();
                    let container = podspec.containers.first_mut().unwrap();

                    container.name = "emb-server".to_string();
                    container.args = Some(
                        vec![
                            "server",
                            "--embedding-config",
                            "$(EMBEDDING_CONFIG_PATH)",
                            "--global-config",
                            "$(GLOBAL_CONFIG_PATH)",
                            "--replica-index",
                            "$(REPLICA_INDEX)",
                            "--replica-size",
                            "$(REPLICA_SIZE)",
                        ]
                        .into_iter()
                        .map(|x| x.to_string())
                        .collect(),
                    );

                    container.resources = embedding_server.resources.clone();
                    container.volume_mounts = embedding_server.volumeMounts.clone();

                    container.image = embedding_server.image.clone();
                    if container.image.is_none() {
                        container.image = Some(String::from(DEFAULT_CUDA_IMAGE));
                    }

                    let env = container
                        .env
                        .as_mut()
                        .expect("no env in a persia podspec template");
                    env.push(EnvVar {
                        name: String::from("REPLICA_INDEX"),
                        value: Some(replica_idx.to_string()),
                        ..EnvVar::default()
                    });

                    env.push(EnvVar {
                        name: String::from("REPLICA_SIZE"),
                        value: Some(embedding_server.replicas.to_string()),
                        ..EnvVar::default()
                    });

                    if let Some(e) = &embedding_server.env {
                        e.iter().for_each(|env_var| {
                            env.push(env_var.clone());
                        });
                    }

                    pod.metadata.name = Some(get_emb_server_pod_name(job_name, replica_idx));
                    pod
                })
                .collect();

            results.append(&mut emb_server_spec);
        }

        if let Some(middleware_server) = &self.middlewareServer {
            let mut middleware_server_spec: Vec<Pod> = (0..middleware_server.replicas)
                .into_iter()
                .map(|replica_idx| {
                    let mut pod = self.gen_pod_template(job_name, namespace);
                    let podspec = pod.spec.as_mut().unwrap();
                    let container = podspec.containers.first_mut().unwrap();

                    container.name = "middleware-server".to_string();
                    container.args = Some(
                        vec![
                            "middleware",
                            "--embedding-config",
                            "$(EMBEDDING_CONFIG_PATH)",
                            "--global-config",
                            "$(GLOBAL_CONFIG_PATH)",
                            "--replica-index",
                            "$(REPLICA_INDEX)",
                            "--replica-size",
                            "$(REPLICA_SIZE)",
                        ]
                        .into_iter()
                        .map(|x| x.to_string())
                        .collect(),
                    );

                    container.resources = middleware_server.resources.clone();
                    container.volume_mounts = middleware_server.volumeMounts.clone();

                    container.image = middleware_server.image.clone();
                    if container.image.is_none() {
                        container.image = Some(String::from(DEFAULT_CUDA_IMAGE));
                    }

                    let env = container
                        .env
                        .as_mut()
                        .expect("no env in a persia podspec template");
                    env.push(EnvVar {
                        name: String::from("REPLICA_INDEX"),
                        value: Some(replica_idx.to_string()),
                        ..EnvVar::default()
                    });

                    env.push(EnvVar {
                        name: String::from("REPLICA_SIZE"),
                        value: Some(middleware_server.replicas.to_string()),
                        ..EnvVar::default()
                    });

                    if let Some(e) = &middleware_server.env {
                        e.iter().for_each(|env_var| {
                            env.push(env_var.clone());
                        });
                    }

                    pod.metadata.name = Some(get_mid_server_pod_name(job_name, replica_idx));
                    pod
                })
                .collect();

            results.append(&mut middleware_server_spec);
        }

        if let Some(trainer) = &self.trainer {
            let mut trainer_spec: Vec<Pod> = (0..trainer.replicas)
                .into_iter()
                .map(|replica_idx| {
                    let mut pod = self.gen_pod_template(job_name, namespace);
                    let podspec = pod.spec.as_mut().unwrap();

                    let container = podspec.containers.first_mut().unwrap();

                    container.name = "trainer".to_string();
                    container.args = Some(
                        vec![
                            "trainer",
                            "$(TRAINER_PY_ENTRY_PATH)",
                            "--gpu-num",
                            "$(NPROC_PER_NODE)",
                            "--nnodes",
                            "$(REPLICA_SIZE)",
                            "--node-rank",
                            "$(REPLICA_INDEX)",
                        ]
                        .into_iter()
                        .map(|x| x.to_string())
                        .collect(),
                    );

                    container.resources = trainer.resources.clone();
                    container.volume_mounts = trainer.volumeMounts.clone();

                    container.image = trainer.image.clone();
                    if container.image.is_none() {
                        container.image = Some(String::from(DEFAULT_CUDA_IMAGE));
                    }

                    let env = container
                        .env
                        .as_mut()
                        .expect("no env in a persia podspec template");
                    env.push(EnvVar {
                        name: String::from("REPLICA_INDEX"),
                        value: Some(replica_idx.to_string()),
                        ..EnvVar::default()
                    });
                    env.push(EnvVar {
                        name: String::from("REPLICA_SIZE"),
                        value: Some(trainer.replicas.to_string()),
                        ..EnvVar::default()
                    });
                    env.push(EnvVar {
                        name: String::from("NPROC_PER_NODE"),
                        value: Some(trainer.nprocPerNode.to_string()),
                        ..EnvVar::default()
                    });
                    env.push(EnvVar {
                        name: String::from("TRAINER_PY_ENTRY_PATH"),
                        value: self.trainerPyEntryPath.clone(),
                        ..EnvVar::default()
                    });

                    if let Some(e) = &trainer.env {
                        e.iter().for_each(|env_var| {
                            env.push(env_var.clone());
                        });
                    }

                    pod.metadata.name = Some(get_trainer_pod_name(job_name, replica_idx));
                    pod
                })
                .collect();

            results.append(&mut trainer_spec);
        }

        if let Some(dataloader) = &self.dataloader {
            let mut dataloader_spec: Vec<Pod> = (0..dataloader.replicas)
                .into_iter()
                .map(|replica_idx| {
                    let mut pod = self.gen_pod_template(job_name, namespace);
                    let podspec = pod.spec.as_mut().unwrap();
                    let container = &mut podspec
                        .containers
                        .first_mut()
                        .expect("no containers in a persia podspec template");

                    container.name = "dataloader".to_string();
                    container.args = Some(
                        vec![
                            "compose",
                            "$(DATALOADER_PY_ENTRY_PATH)",
                            "--replica-index",
                            "$(REPLICA_INDEX)",
                            "--replica-size",
                            "$(REPLICA_SIZE)",
                        ]
                        .into_iter()
                        .map(|x| x.to_string())
                        .collect(),
                    );

                    container.resources = dataloader.resources.clone();
                    container.volume_mounts = dataloader.volumeMounts.clone();

                    container.image = dataloader.image.clone();
                    if container.image.is_none() {
                        container.image = Some(String::from(DEFAULT_CUDA_IMAGE));
                    }

                    let env = container
                        .env
                        .as_mut()
                        .expect("no env in a persia podspec template");
                    env.push(EnvVar {
                        name: String::from("REPLICA_INDEX"),
                        value: Some(replica_idx.to_string()),
                        ..EnvVar::default()
                    });
                    env.push(EnvVar {
                        name: String::from("REPLICA_SIZE"),
                        value: Some(dataloader.replicas.to_string()),
                        ..EnvVar::default()
                    });
                    env.push(EnvVar {
                        name: String::from("DATALOADER_PY_ENTRY_PATH"),
                        value: self.dataLoaderPyEntryPath.clone(),
                        ..EnvVar::default()
                    });

                    if let Some(e) = &dataloader.env {
                        e.iter().for_each(|env_var| {
                            env.push(env_var.clone());
                        });
                    }

                    pod.metadata.name = Some(get_dataloader_pod_name(job_name, replica_idx));
                    pod
                })
                .collect();

            results.append(&mut dataloader_spec);
        }

        if self.enableMetrics.unwrap_or(true) {
            let service_name = get_metrics_gateway_service_name(job_name);

            let mut metrics_labels: BTreeMap<String, String> = BTreeMap::new();
            metrics_labels.insert("app".to_owned(), job_name.to_owned());
            metrics_labels.insert("service".to_owned(), service_name);

            let metrics_pod = Pod {
                metadata: ObjectMeta {
                    name: Some(get_metrics_gateway_pod_name(job_name)),
                    namespace: Some(namespace.to_owned()),
                    labels: Some(metrics_labels),
                    ..ObjectMeta::default()
                },
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: String::from("pushgateway"),
                        image: Some(String::from("prom/pushgateway:latest")),
                        image_pull_policy: Some(String::from("IfNotPresent")),
                        ..Container::default()
                    }],
                    volumes: self.volumes.clone(),
                    ..PodSpec::default()
                }),
                ..Pod::default()
            };

            results.push(metrics_pod);
        }

        results
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[allow(non_snake_case)]
pub struct EmbeddingSpec {
    pub replicas: usize,
    pub resources: Option<ResourceRequirements>,
    pub volumeMounts: Option<Vec<VolumeMount>>,
    pub env: Option<Vec<EnvVar>>,
    pub image: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[allow(non_snake_case)]
pub struct MiddlewareSpec {
    pub replicas: usize,
    pub resources: Option<ResourceRequirements>,
    pub volumeMounts: Option<Vec<VolumeMount>>,
    pub env: Option<Vec<EnvVar>>,
    pub image: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[allow(non_snake_case)]
pub struct TrainerSpec {
    pub replicas: usize,
    pub nprocPerNode: usize,
    pub resources: Option<ResourceRequirements>,
    pub volumeMounts: Option<Vec<VolumeMount>>,
    pub env: Option<Vec<EnvVar>>,
    pub image: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, JsonSchema)]
#[allow(non_snake_case)]
pub struct DataLoaderSpec {
    pub replicas: usize,
    pub resources: Option<ResourceRequirements>,
    pub volumeMounts: Option<Vec<VolumeMount>>,
    pub env: Option<Vec<EnvVar>>,
    pub image: Option<String>,
}