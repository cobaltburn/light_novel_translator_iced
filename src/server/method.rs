use iced::Task;
use std::iter;

const BATCH_SIZE: usize = 6;

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub enum Method {
    #[default]
    History,
    Chain,
    Batch,
}

impl Method {
    pub fn join_tasks<T: 'static>(self, tasks: Vec<Task<T>>) -> Task<T> {
        match self {
            Method::Batch => {
                let mut iter = tasks.into_iter();
                iter::from_fn(|| {
                    let chunk: Vec<_> = iter.by_ref().take(BATCH_SIZE).collect();
                    (!chunk.is_empty()).then_some(chunk)
                })
                .map(Task::batch)
                .fold(Task::none(), Task::chain)
            }
            Method::History | Method::Chain => tasks.into_iter().fold(Task::none(), Task::chain),
        }
    }
}
