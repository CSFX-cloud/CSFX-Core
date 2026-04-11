<script lang="ts">
  import * as Card from '$lib/components/ui/card/index.js';
  import {
    FieldGroup,
    Field,
    FieldLabel,
    FieldDescription,
  } from '$lib/components/ui/field/index.js';
  import { Input } from '$lib/components/ui/input/index.js';
  import { Button } from '$lib/components/ui/button/index.js';
  import { Alert, AlertDescription } from '$lib/components/ui/alert/index.js';
  import { cn } from '$lib/utils.js';
  import type { HTMLAttributes } from 'svelte/elements';
  import { goto } from '$app/navigation';
  import { AuthService } from '$lib/services/auth';
  import { authStore } from '$lib/stores/auth';

  let { class: className, ...restProps }: HTMLAttributes<HTMLDivElement> = $props();

  // Browser-compatible UUID generation
  function generateUUID() {
    if (typeof crypto !== 'undefined' && crypto.randomUUID) {
      return crypto.randomUUID();
    }
    // Fallback for browsers without crypto.randomUUID
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
      const r = (Math.random() * 16) | 0;
      const v = c === 'x' ? r : (r & 0x3) | 0x8;
      return v.toString(16);
    });
  }

  const id = generateUUID();

  let username = $state('');
  let password = $state('');
  let isLoading = $state(false);
  let errorMessage = $state('');

  async function handleSubmit(event: Event) {
    event.preventDefault();

    if (!username.trim() || !password) {
      errorMessage = 'Username and password are required';
      return;
    }

    isLoading = true;
    errorMessage = '';

    try {
      const response = await AuthService.login(
        username,
        password,
        undefined // No 2FA code on initial login
      );

      // Check if password change is required
      if (response.force_password_change) {
        // Store token temporarily and redirect to password change
        authStore.login(
          {
            id: response.user_id,
            username: response.username,
            force_password_change: true,
            two_factor_enabled: response.two_factor_enabled,
          },
          response.token
        );

        await fetch('/api/set-auth-cookie', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ token: response.token }),
        });

        goto('/change-password');
        return;
      }

      // Update auth store
      authStore.login(
        {
          id: response.user_id,
          username: response.username,
          force_password_change: false,
          two_factor_enabled: response.two_factor_enabled,
        },
        response.token
      );

      console.log('[FinanceVault] Logging in user', {
        username: response.username,
        userId: response.user_id,
      });

      // Set cookie via API
      const cookieResponse = await fetch('/api/set-auth-cookie', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ token: response.token }),
      });

      if (!cookieResponse.ok) {
        console.error('Failed to set auth cookie:', await cookieResponse.text());
        errorMessage = 'Failed to set authentication cookie';
        isLoading = false;
        return;
      }

      console.log('[FinanceVault] Login successful, redirecting to home');

      // Force reload to ensure auth state is fully updated
      window.location.href = '/';
    } catch (error) {
      if (error instanceof Error && error.message === '2FA_REQUIRED') {
        // Store credentials in sessionStorage and redirect to OTP page
        sessionStorage.setItem('totp_pending', JSON.stringify({ username, password }));
        goto('/otp');
      } else {
        errorMessage = error instanceof Error ? error.message : 'Login failed';
        isLoading = false;
      }
    }
  }
</script>

<div class={cn('min-h-screen flex', className)} {...restProps}>
  <!-- Left Side - Login Form -->
  <div class="flex-1 flex items-center justify-center p-8 bg-background">
    <div class="w-full max-w-md space-y-8">
      <div class="text-center">
        <h1 class="text-3xl font-bold tracking-tight">Willkommen zurück</h1>
        <p class="text-muted-foreground mt-2">Melden Sie sich in Ihrem CSFX-Core Konto an</p>
      </div>

      <form onsubmit={handleSubmit} class="space-y-6">
        {#if errorMessage}
          <Alert variant="destructive">
            <AlertDescription>{errorMessage}</AlertDescription>
          </Alert>
        {/if}

        <FieldGroup class="space-y-4">
          <Field>
            <FieldLabel for="username-{id}" class="text-sm font-medium">Benutzername</FieldLabel>
            <Input
              id="username-{id}"
              type="text"
              placeholder="Geben Sie Ihren Benutzernamen ein"
              bind:value={username}
              required
              disabled={isLoading}
              class="mt-1"
            />
          </Field>

          <Field>
            <div class="flex items-center justify-between">
              <FieldLabel for="password-{id}" class="text-sm font-medium">Passwort</FieldLabel>
              <a
                href="##"
                class="text-sm text-primary hover:text-primary/80 underline-offset-2 hover:underline"
              >
                Passwort vergessen?
              </a>
            </div>
            <Input
              id="password-{id}"
              type="password"
              placeholder="Geben Sie Ihr Passwort ein"
              bind:value={password}
              required
              disabled={isLoading}
              class="mt-1"
            />
          </Field>

          <Button type="submit" class="w-full h-11" disabled={isLoading}>
            {#if isLoading}
              <div class="animate-spin rounded-full h-4 w-4 border-b-2 border-current mr-2"></div>
            {/if}
            {isLoading ? 'Anmeldung läuft...' : 'Anmelden'}
          </Button>
        </FieldGroup>
      </form>
    </div>
  </div>

  <!-- Right Side - Animated Background with Features -->
  <div class="flex-1 relative hidden lg:flex flex-col overflow-hidden">
    <!-- Animated Background Container -->
    <div class="absolute inset-0 rounded-l-3xl overflow-hidden">
      <!-- Gradient SVG Background as Image -->
      <img src="/Gradientsv2.svg" alt="" class="gradient-svg" />
    </div>

    <!-- Logo Overlay -->
    <div class="absolute inset-0 flex items-center justify-center z-10">
      <div class="text-center text-white">
        <img
          src="/logos/CSF_Logo.png"
          alt="CSF-Core Logo"
          class="mx-auto mb-4 w-200 h-200 md:w-200 h-200 lg:w-[240px] h-[240px]"
        />

        <h2 class="text-4xl font-bold mb-4 text-shadow-glow">CSFX-Core</h2>
        <div class="space-y-2 text-lg text-shadow-glow">The AI-Ready Business Platform</div>
      </div>
    </div>
  </div>
</div>

<style>
  .gradient-svg {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    z-index: -1;
  }

  .text-shadow-glow {
    text-shadow: 0 0 30px var(--primary-foreground);
  }
</style>
