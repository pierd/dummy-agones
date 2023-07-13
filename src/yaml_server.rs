use axum::{extract::State, routing::IntoMakeService};
use axum_yaml::Yaml;

pub trait Store<T> {
    fn get(&self) -> T;
    fn set(&self, value: T);
}

async fn load<T, S: Store<T>>(State(state): State<S>) -> Yaml<T> {
    Yaml(state.get())
}

async fn save<T, S: Store<T>>(State(state): State<S>, Yaml(value): Yaml<T>) -> Yaml<T> {
    state.set(value);
    Yaml(state.get())
}

pub fn create_service<T, S>(store: S) -> IntoMakeService<axum::Router>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
    S: Store<T> + Clone + Send + Sync + 'static,
{
    axum::Router::new()
        .route("/", axum::routing::get(load::<T, S>))
        .route("/", axum::routing::post(save::<T, S>))
        .with_state(store)
        .into_make_service()
}
