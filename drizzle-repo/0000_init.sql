CREATE TABLE `commits` (
	`hash` text PRIMARY KEY NOT NULL,
	`time` integer NOT NULL,
	`parents` text NOT NULL,
	`subject` text NOT NULL,
	`author` text NOT NULL,
	`body` text,
	`expanded` integer DEFAULT 0 NOT NULL
);
--> statement-breakpoint
CREATE INDEX `commit_time_hash` ON `commits` ("time" DESC,`hash`);--> statement-breakpoint
CREATE TABLE `edge` (
	`view_id` text NOT NULL,
	`from` integer NOT NULL,
	`to` integer NOT NULL,
	`line` text NOT NULL,
	`points` text NOT NULL,
	PRIMARY KEY(`view_id`, `from`, `to`)
);
--> statement-breakpoint
CREATE TABLE `meta` (
	`key` text PRIMARY KEY NOT NULL,
	`value` text NOT NULL
);
--> statement-breakpoint
CREATE TABLE `node` (
	`view_id` text NOT NULL,
	`row` integer NOT NULL,
	`hash` text NOT NULL,
	`lane` integer NOT NULL,
	`line` text NOT NULL,
	`pushed` integer NOT NULL,
	`refs` text NOT NULL,
	PRIMARY KEY(`view_id`, `row`)
);
--> statement-breakpoint
CREATE INDEX `node_hash` ON `node` (`view_id`,`hash`);--> statement-breakpoint
CREATE TABLE `pending` (
	`hash` text PRIMARY KEY NOT NULL,
	`time` integer NOT NULL
);
--> statement-breakpoint
CREATE TABLE `views` (
	`id` text PRIMARY KEY NOT NULL,
	`only_lines` text NOT NULL,
	`total` integer NOT NULL,
	`lanes` integer NOT NULL,
	`version` integer NOT NULL
);
