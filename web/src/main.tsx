import { QueryClientProvider } from "@tanstack/react-query";
import { createRouter, RouterProvider } from "@tanstack/react-router";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { createQueryClient } from "./api/client";
import { sessionQuery } from "./api/queries";
import { NotFound, RouteError } from "./components/route-error";
import { getLocale } from "./paraglide/runtime.js";
import { routeTree } from "./routeTree.gen";
import "./styles.css";

// The session ended: forget it, so the login page asks the server again, and go there.
const queryClient = createQueryClient(() => {
  queryClient.removeQueries({ queryKey: sessionQuery.queryKey });
  void router.navigate({ to: "/login" });
});
const router = createRouter({
  routeTree,
  context: { queryClient },
  defaultPreload: "intent",
  defaultErrorComponent: RouteError,
  defaultNotFoundComponent: NotFound,
});

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

// index.html says "de"; screen readers need the language actually shown. setLocale reloads the
// page, so setting it once here is enough.
document.documentElement.lang = getLocale();

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </StrictMode>,
  );
}
