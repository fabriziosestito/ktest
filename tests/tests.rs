use k8s_openapi::api::core::v1::Pod;
use ktest::ktest;
use kube::Api;

#[ktest]
#[tokio::test]
async fn test_ktest() {
    let version = client.apiserver_version().await.unwrap();

    assert_eq!(version.git_version, "v1.29.2");
}

#[ktest(fixtures = ["tests/pod.yaml"])]
#[tokio::test]
async fn test_ktest_with_fixtures() {
    let api: Api<Pod> = Api::default_namespaced(client);

    let pod = api.get("test-pod").await.unwrap();

    assert_eq!(pod.metadata.name.unwrap(), "test-pod");
    assert_eq!(pod.metadata.namespace.unwrap(), "default");
    assert_eq!(pod.spec.unwrap().containers[0].name, "busybox");
}
