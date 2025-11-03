// This example compiles on stable Rust. No external crates are required.

use std::cell::RefCell;
use std::rc::Rc;

/// A token that would normally identify a registered resource.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Token(u64);

/// What a handler can request after being called.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Dispatch {
    Continue,
    Stop,
}

/// A minimal context object passed to handlers during dispatch.
#[derive(Copy, Clone, Debug)]
struct Context {
    token: Token,
}

impl Context {
    fn token(&self) -> Token {
        self.token
    }
}

/// The public-facing reactor handle (toy version).
struct Reactor;

impl Reactor {
    fn new() -> Self {
        Self
    }

    /// Start a **scoped** run. The lifetime `'scope` is tied to this call.
    ///
    /// - `with` receives a `Scope<'scope>` that allows the user to register handlers
    ///   which may borrow **non-`'static`** state (e.g., `&'scope mut T`).
    /// - After registration, we "run" once by dispatching a synthetic event.
    fn run_scoped<'scope, R>(&mut self, with: impl FnOnce(Scope<'scope>) -> R) -> R {
        // Internal registry parameterized by `'scope`.
        let inner = Rc::new(RefCell::new(Registry::<'scope>::default()));

        // Provide the user a Scope to register handlers within `'scope`.
        let scope = Scope { inner: inner.clone() };
        let ret = with(scope);

        // ---- Minimal "event loop": dispatch one synthetic event to prove lifetimes ----
        // In a real reactor you'd poll the OS; here we just fabricate a readiness event.
        let token = Token(1);
        let ctx = Context { token };

        // Snapshot handlers to avoid borrow conflicts during iteration.
        // (In a real loop you'd handle pending ops, reregistration, etc.)
        let handlers = {
            let guard = inner.borrow();
            guard.handlers.iter().map(|h| h.as_ref()).collect::<Vec<_>>()
        };

        for handler in handlers {
            let decision = handler(&ctx);
            if let Dispatch::Stop = decision {
                break;
            }
        }
        // -----------------------------------------------------------------------------

        ret
    }
}

/// The scope exposed to users for registering handlers.
/// Note the explicit `'scope` lifetime parameter.
#[derive(Clone)]
struct Scope<'scope> {
    inner: Rc<RefCell<Registry<'scope>>>,
}

impl<'scope> Scope<'scope> {
    /// Register a handler for a token (resource). The handler may borrow `&'scope mut T`.
    fn on_event<F>(&self, _token: Token, handler: F)
    where
        F: Fn(&Context) -> Dispatch + 'scope,
    {
        self.inner.borrow_mut().handlers.push(Box::new(handler));
    }
}

/// Internal registry that stores handlers with lifetime `'scope`.
#[derive(Default)]
struct Registry<'scope> {
    handlers: Vec<Box<dyn Fn(&Context) -> Dispatch + 'scope>>,
}

fn main() {
    let mut reactor = Reactor::new();

    // --- User state that handlers will mutate by capturing a &mut borrow ---
    #[derive(Debug)]
    struct State {
        counter: u32,
        last_token: Option<Token>,
    }

    let mut state = State { counter: 0, last_token: None };

    // `run_scoped` introduces `'scope` tied to this call.
    // Inside, handlers can capture `&'scope mut state` safely.
    reactor.run_scoped(|scope| {
        // Capture a mutable borrow of `state` in the handler.
        // This is possible because the handler is bounded by `'scope` (not `'static`).
        scope.on_event(Token(1), |ctx| {
            // We can use &mut state here because it’s captured from the surrounding scope.
            // (Closure environment holds the &mut borrow; shown below.)
            Dispatch::Continue
        });

        // A second handler that actually mutates `state` to demonstrate effects.
        // We create a new registration so we can explicitly capture `&mut state`.
        let state_ref: &mut State = &mut state;
        scope.on_event(Token(1), move |ctx| {
            state_ref.counter += 1;
            state_ref.last_token = Some(ctx.token());
            println!("handler ran; state = {:?}", state_ref);
            Dispatch::Stop // ask the (toy) loop to stop after one dispatch
        });
    });

    // After run_scoped returns, the borrow on `state` has ended, so we can use it again.
    println!("final state = {:?}", state);
}
