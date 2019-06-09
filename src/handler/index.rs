use gotham::state::State;

pub fn handler(state: State) -> (State, String) {
    (state, String::from("index goes here"))
}
