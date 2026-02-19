#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Reading,
    Writing,
    Closed,
}
