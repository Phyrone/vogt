CREATE TABLE `commit_node` (
	`repo_id` text NOT NULL,
	`hash` blob NOT NULL,
	`time` integer NOT NULL,
	`parents` blob NOT NULL,
	PRIMARY KEY(`repo_id`, `hash`),
	FOREIGN KEY (`repo_id`) REFERENCES `repo`(`id`) ON UPDATE no action ON DELETE cascade
);
