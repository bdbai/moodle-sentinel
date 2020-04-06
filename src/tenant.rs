#[derive(Clone, Copy, Debug)]
pub enum Tenant {
    #[allow(unused)]
    SenderSelf,
    Group(i64),
}
