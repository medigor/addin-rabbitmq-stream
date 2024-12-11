use std::{cell::RefCell, collections::HashMap};

use rabbitmq_stream_client::Environment;

thread_local! {
    static ENVIRONMENTS: RefCell<Option<HashMap<i32, Environment>>> = const { RefCell::new(None) };
    static COUNTER: RefCell<i32> = const { RefCell::new(1) };
}

pub fn insert_environment(env: Environment) -> i32 {
    let id = COUNTER.with_borrow_mut(|x| {
        let id = *x;
        *x += 1;
        id
    });
    ENVIRONMENTS.with_borrow_mut(|x| x.get_or_insert_default().insert(id, env));
    id
}

pub fn remove_environment(id: i32) {
    ENVIRONMENTS.with_borrow_mut(|x| {
        if let Some(hm) = x {
            hm.remove(&id);
            if hm.is_empty() {
                x.take();
            }
        }
    });
}

pub fn with_environment<F, R>(id: i32, f: F) -> Option<R>
where
    F: FnOnce(&mut Environment) -> R,
{
    ENVIRONMENTS.with_borrow_mut(|x| {
        let Some(hm) = x else {
            return None;
        };
        let env = hm.get_mut(&id)?;
        Some(f(env))
    })
}
