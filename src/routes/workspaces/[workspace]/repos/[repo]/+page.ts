import { resolve } from '$app/paths';
import { redirect } from '@sveltejs/kit';
import type { PageLoad } from './$types';

// history is the repository's home until it has more than one view
export const load: PageLoad = ({ params }) => {
	redirect(307, resolve('/workspaces/[workspace]/repos/[repo]/history', params));
};
