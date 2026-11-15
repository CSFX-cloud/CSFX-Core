import type { UserProfile } from '$lib/auth/api';

declare global {
	namespace App {
		interface Locals {
			user: UserProfile | null;
		}
		interface PageData {
			user: UserProfile | null;
		}
	}
}

export {};
