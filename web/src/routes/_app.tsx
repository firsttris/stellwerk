import { createFileRoute, Outlet, redirect } from "@tanstack/react-router";
import { sessionQuery } from "../api/queries";
import { Layout } from "../components/layout";

/** Everything but the login page: only for the logged-in admin. */
export const Route = createFileRoute("/_app")({
  beforeLoad: async ({ context }) => {
    // fetchQuery, not ensureQueryData: asks the server again once the session is a minute old.
    const session = await context.queryClient.fetchQuery(sessionQuery);
    if (!session.loggedIn) throw redirect({ to: "/login" });
  },
  component: () => (
    <Layout>
      <Outlet />
    </Layout>
  ),
});
