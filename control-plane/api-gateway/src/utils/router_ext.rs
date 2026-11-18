use axum::Router;
use tracing::info;

const HOST: &str = "http://localhost:8000";

pub trait RouterExt<S> {
    fn logged_nest(self, path: &str, router: Router<S>) -> Self;
}

impl<S> RouterExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn logged_nest(self, path: &str, router: Router<S>) -> Self {
        // This is a simplification. A real implementation would need to inspect the nested router,
        // but Axum's API doesn't easily expose registered routes.
        // For this implementation, we log the base path of the nested router.
        let full_path = format!("{}{}", HOST, path);
        info!(target: "axum_monitor", "Nesting routes under {} => ONLINE", full_path);
        self.nest(path, router)
    }
}
