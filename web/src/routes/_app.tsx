import { createFileRoute, Outlet, redirect } from "@tanstack/react-router";
import { sessionQuery } from "../api/queries";
import { Layout } from "../components/layout";

/** Everything but the login page: only for the logged-in admin. */
export const Route = createFileRoute("/_app")({
  beforeLoad: async ({ context }) => {
    const session = await context.queryClient.ensureQueryData(sessionQuery);
    if (!session.loggedIn) throw redirect({ to: "/login" });
  },
  component: () => (
    <Layout>
      <Outlet />
    </Layout>
  ),
});
