use std::{cell::RefCell, collections::HashMap};

use rabbitmq_stream_client::Environment;

thread_local! {
    static ENVIRONMENTS: RefCell<HashMap<i32, Environment>> = RefCell::new(HashMap::new());
    static COUNTER: RefCell<i32> = const { RefCell::new(1) };
}

pub fn insert_environment(env: Environment) -> i32 {
    let id = COUNTER.with_borrow_mut(|x| {
        let id = *x;
        *x += 1;
        id
    });
    ENVIRONMENTS.with_borrow_mut(|x| x.insert(id, env));
    id
}

pub fn remove_environment(id: i32) {
    ENVIRONMENTS.with_borrow_mut(|x| {
        x.remove(&id);
        if x.is_empty() {
            x.shrink_to_fit();
        }
    });
}

pub fn with_environment<F, R>(id: i32, f: F) -> Option<R>
where
    F: FnOnce(&mut Environment) -> R,
{
    ENVIRONMENTS.with_borrow_mut(|x| {
        let env = x.get_mut(&id)?;
        Some(f(env))
    })
}
