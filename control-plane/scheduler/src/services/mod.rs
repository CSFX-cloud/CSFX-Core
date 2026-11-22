pub mod attachment_guard;
pub mod bucket_bindings;
pub mod compose_parser;
pub mod compose_redact;
pub mod gateway_notify;
pub mod guest_ip;
pub mod guest_ip_allocation;
pub mod image_resolver;
pub mod placement;
pub mod port_allocator;
pub mod rightsizing;
pub mod rolling;
pub mod scheduler;
pub mod stack_scheduling;
pub mod stack_volumes;

pub(crate) fn failed<E: std::fmt::Display>(action: &'static str) -> impl FnOnce(E) -> String {
    move |cause| format!("Failed to {}: {}", action, cause)
}
