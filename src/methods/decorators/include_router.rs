use crate::Client;
use crate::handlers::{HandlerRef, Router};

impl Client {
    /// Includes `router`'s handlers, middleware and values, as one entry in the client's groups: group 0 unless the
    /// returned [`HandlerRef`] says otherwise. When one of its handlers matches an update, or its outer middleware
    /// keeps it, the router has taken its group's turn, as a handler would. Its handlers keep the ids its
    /// [`RouteRef`](crate::handlers::RouteRef)s gave, for [`remove_handler`](Client::remove_handler), and are listed
    /// by [`handlers`](Client::handlers) in its place; removing the returned id removes the whole router.
    pub fn include_router(&self, router: Router) -> HandlerRef<'_> {
        self.mount(router)
    }
}
