use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, ItemFn, Lit, Meta, MetaNameValue};

#[proc_macro_attribute]
pub fn ktest(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_attrs = &input_fn.attrs;
    let fn_block = &input_fn.block;

    // Parse attributes to extract fixture file names
    let attr_string = attr.to_string();
    let mut fixture_files = Vec::new();

    if attr_string.starts_with("fixtures") {
        if let Some(start) = attr_string.find('[') {
            if let Some(end) = attr_string.find(']') {
                let list_str = &attr_string[start + 1..end];
                fixture_files = list_str
                    .split(',')
                    .map(|s| s.trim().trim_matches('"').to_string())
                    .collect();
            }
        }
    }

    let fixture_code = if fixture_files.is_empty() {
        quote! {} // No-op if no fixtures are provided
    } else {
        let fixture_apply_statements = fixture_files.iter().map(|file| {
            quote! {
                use kube::api::ResourceExt;
                let ssapply = kube::api::PatchParams::apply("ktest").force();
                let discovery = kube::discovery::Discovery::new(client.clone()).run().await.unwrap();

                let yaml = std::fs::read_to_string(#file).expect("Failed to read fixture file");
                let doc: kube::api::DynamicObject = serde_yaml::from_str(&yaml).expect("Invalid YAML format");
                let namespace = doc.metadata.namespace.as_deref();

                let gvk = kube::api::GroupVersionKind::try_from(&doc.clone().types.unwrap()).unwrap();

                let name = doc.clone().name_any();
                if let Some((ar, caps)) = discovery.resolve_gvk(&gvk) {
                    let api: kube::api::Api<kube::api::DynamicObject> = if caps.scope == kube::discovery::Scope::Cluster || false {
                        kube::api::Api::all_with(client.clone(), &ar)
                    } else if let Some(namespace) = namespace {
                         kube::api::Api::namespaced_with(client.clone(), namespace, &ar)
                    } else {
                         kube::api::Api::default_namespaced_with(client.clone(), &ar)
                    };
                    let data: serde_json::Value = serde_json::to_value(&doc).unwrap();
                    let _r = api.patch(&name, &ssapply, &kube::api::Patch::Apply(data)).await.unwrap();
                } else {
                    panic!("Cannot resolve GVK for {:?}", gvk);
                }
            }
        });

        quote! {
            use kube::api::PostParams;
            use serde_yaml;
            #(#fixture_apply_statements)*
        }
    };

    let output = quote! {
        #(#fn_attrs)*
        async fn #fn_name() {
            use testcontainers_modules::{kwok::KwokCluster, testcontainers::runners::AsyncRunner};
            use kube::{
                client::Client,
                config::{AuthInfo, Cluster, KubeConfigOptions, Kubeconfig, NamedAuthInfo, NamedCluster},
                Config,
            };
            use rustls::crypto::CryptoProvider;

            let cluster_name = "kwok-kwok";
            let context_name = "kwok-kwok";
            let cluster_user = "kwok-kwok";

            if CryptoProvider::get_default().is_none() {
                rustls::crypto::ring::default_provider()
                    .install_default()
                    .expect("Error initializing rustls provider");
            }

            let node = KwokCluster::default().start().await.unwrap();
            let host_port = node.get_host_port_ipv4(8080).await.unwrap();

            let kubeconfig = Kubeconfig {
                clusters: vec![NamedCluster {
                    name: cluster_name.to_owned(),
                    cluster: Some(Cluster {
                        server: Some(format!("http://localhost:{host_port}")),
                        ..Default::default()
                    }),
                }],
                contexts: vec![kube::config::NamedContext {
                    name: context_name.to_owned(),
                    context: Some(kube::config::Context {
                        cluster: cluster_name.to_owned(),
                        user: Some(cluster_user.to_owned()),
                        ..Default::default()
                    }),
                }],
                auth_infos: vec![NamedAuthInfo {
                    name: cluster_user.to_owned(),
                    auth_info: Some(AuthInfo::default()),
                }],
                current_context: Some(context_name.to_owned()),
                ..Default::default()
            };
            let kubeconfigoptions = KubeConfigOptions {
                context: Some(context_name.to_owned()),
                cluster: Some(cluster_name.to_owned()),
                user: None,
            };

            let config = Config::from_custom_kubeconfig(kubeconfig, &kubeconfigoptions)
                .await
                .unwrap();

            let client = Client::try_from(config).unwrap();

            #fixture_code

            async fn inner(client: kube::Client) {
                #fn_block
            }
            inner(client).await;
        }
    };

    output.into()
}
