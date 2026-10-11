/** A repository the app knows about. The id is issued by the backend and is what routes carry. */
export interface Repo {
	id: string;
	name: string;
	path: string;
}

/** A named group of repositories. */
export interface Workspace {
	id: string;
	name: string;
	repos: Repo[];
}
