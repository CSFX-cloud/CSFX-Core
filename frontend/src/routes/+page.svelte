<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { authStore } from '$lib/stores/auth';
</script>

<div class="flex items-center justify-center h-screen">
  <div class="text-center">
    <h1 class="text-4xl font-bold">FinanceVault</h1>
    <p class="mt-4 text-muted-foreground"></p>
  </div>
</div>
