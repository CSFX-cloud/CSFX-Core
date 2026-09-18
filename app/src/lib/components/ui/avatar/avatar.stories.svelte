<script module lang="ts">
	import { defineMeta } from "@storybook/addon-svelte-csf";
	import { Avatar, AvatarImage, AvatarFallback, AvatarGroup, AvatarGroupCount } from "./index.js";

	const { Story } = defineMeta({
		title: "ui/Avatar",
		component: Avatar,
		tags: ["autodocs"],
	});
</script>

<Story name="WithImage">
	<Avatar>
		<AvatarImage src="https://github.com/shadcn.png" alt="user avatar" />
		<AvatarFallback>CS</AvatarFallback>
	</Avatar>
</Story>

<Story name="FallbackOnly">
	<Avatar>
		<AvatarImage src="https://broken.invalid/none.png" alt="user avatar" />
		<AvatarFallback>CS</AvatarFallback>
	</Avatar>
</Story>

<Story name="Sizes">
	<div class="flex items-center gap-2">
		<Avatar size="sm">
			<AvatarFallback>SM</AvatarFallback>
		</Avatar>
		<Avatar size="default">
			<AvatarFallback>MD</AvatarFallback>
		</Avatar>
		<Avatar size="lg">
			<AvatarFallback>LG</AvatarFallback>
		</Avatar>
	</div>
</Story>

<Story name="Group">
	<AvatarGroup>
		<Avatar>
			<AvatarFallback>AA</AvatarFallback>
		</Avatar>
		<Avatar>
			<AvatarFallback>BB</AvatarFallback>
		</Avatar>
		<Avatar>
			<AvatarFallback>CC</AvatarFallback>
		</Avatar>
		<AvatarGroupCount>+3</AvatarGroupCount>
	</AvatarGroup>
</Story>
