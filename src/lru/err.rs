use std::fmt::{Display, Formatter, Result};

#[derive(PartialEq, Debug)]
pub struct ArenaOOM;

impl Display for ArenaOOM {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "Arena out of memory.")
    }
}

#[derive(PartialEq, Debug)]
pub enum ListError {
    LinkBroken,
    ListOOM(ArenaOOM),
    ListEmpty,
}

impl Display for ListError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match &self {
            ListError::LinkBroken => write!(f, "Link does not point to a valid location."),
            ListError::ListOOM(arena_oom) => {
                write!(f, "List out of memory: ")?;
                arena_oom.fmt(f)
            }
            ListError::ListEmpty => write!(f, "List is empty."),
        }
    }
}
