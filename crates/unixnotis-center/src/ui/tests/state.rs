use super::UiStateInit;

#[test]
fn constructor_input_contract_lists_every_owned_runtime_resource() {
    fn consume_init(init: UiStateInit) {
        let UiStateInit {
            app: _,
            config: _,
            config_path: _,
            command_tx: _,
            css: _,
            event_tx: _,
            media_handle: _,
            runtime: _,
        } = init;
    }

    std::hint::black_box(consume_init as fn(UiStateInit));
}
