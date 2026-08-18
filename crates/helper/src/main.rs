use helper::HelperApi;

#[tokio::main]
async fn main() {
    let _helper = HelperApi::from_env();
    println!("argus-helper started");
}
