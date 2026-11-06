#[derive(Debug, Clone)]
pub struct PageError {
    pub index: usize,
    pub error: ErrorType,
}

impl PageError {
    pub fn japanese(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Japanese,
        }
    }

    pub fn size(index: usize, size: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Size(size),
        }
    }

    pub fn repeat(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Repeat,
        }
    }

    pub fn copy(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Copy,
        }
    }

    pub fn image(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Image,
        }
    }

    pub fn header(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Header,
        }
    }

    pub fn link(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Link,
        }
    }

    pub fn end(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::End,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ErrorType {
    Japanese,
    Size(usize),
    Repeat,
    Copy,
    Image,
    Header,
    Link,
    End,
}
