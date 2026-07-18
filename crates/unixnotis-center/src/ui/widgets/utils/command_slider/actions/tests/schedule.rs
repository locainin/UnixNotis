use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use super::schedule_command;

#[gtk::test]
fn scheduled_command_coalesces_values_and_clears_pending_state() {
    let pending = Rc::new(RefCell::new(None));
    let pending_value = Rc::new(Cell::new(None));
    let result = Rc::new(Cell::new(None));
    let main_loop = glib::MainLoop::new(None, false);

    let on_complete: Rc<dyn Fn(bool)> = Rc::new({
        let result = result.clone();
        let main_loop = main_loop.clone();
        move |failed| {
            result.set(Some(failed));
            main_loop.quit();
        }
    });

    schedule_command(
        pending.clone(),
        pending_value.clone(),
        "test {value} = 17".to_string(),
        4.0,
        1.0,
        on_complete.clone(),
    );
    schedule_command(
        pending.clone(),
        pending_value.clone(),
        "test {value} = 17".to_string(),
        17.0,
        1.0,
        on_complete,
    );

    let timeout_loop = main_loop.clone();
    glib::timeout_add_local_once(Duration::from_secs(2), move || timeout_loop.quit());
    main_loop.run();

    assert_eq!(result.get(), Some(false));
    assert!(pending.borrow().is_none());
    assert_eq!(pending_value.get(), None);
}
