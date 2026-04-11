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
  import { NativeSelect } from '$lib/components/ui/native-select/index.js';
  import * as InputOTP from '$lib/components/ui/input-otp/index.js';
  import { onMount } from 'svelte';

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

  let isLoading = $state(false);
  let errorMessage = $state('');
  let otp = $state('');
  let selectedMethod = $state('email'); // "email" | "sms" | "authenticator"
  let is2FA = $state(false); // Check if this is for 2FA TOTP
  let username = $state('');
  let password = $state('');

  onMount(() => {
    // Check if we have pending 2FA credentials
    const stored = sessionStorage.getItem('totp_pending');
    if (stored) {
      const data = JSON.parse(stored);
      username = data.username;
      password = data.password;
      is2FA = true;
      selectedMethod = 'authenticator';
    }
  });

  async function handleSubmit(event: Event) {
    event.preventDefault();

    if (!otp || otp.trim().length === 0) {
      errorMessage = 'Bitte geben Sie den Code ein.';
      return;
    }

    if (!is2FA && !selectedMethod) {
      errorMessage = 'Bitte wählen Sie eine Verifizierungsmethode.';
      return;
    }

    isLoading = true;
    errorMessage = '';

    try {
      if (is2FA) {
        // Handle 2FA TOTP verification
        const response = await AuthService.login(username, password, otp.trim());

        // Clear sessionStorage
        sessionStorage.removeItem('totp_pending');

        // Check if password change is required
        if (response.force_password_change) {
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

        await fetch('/api/set-auth-cookie', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ token: response.token }),
        });

        goto('/');
      } else {
        // Handle regular OTP verification (email/sms)
        const response = await AuthService.verifyOtp({
          method: selectedMethod,
          code: otp.trim(),
        });

        if (response?.token && response?.user_id) {
          authStore.login({ id: response.user_id, username: response.username }, response.token);

          await fetch('/api/set-auth-cookie', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ token: response.token }),
          });

          goto('/');
        } else {
          throw new Error('Verifizierung fehlgeschlagen');
        }
      }
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'Verifizierung fehlgeschlagen';
      isLoading = false;
    }
  }

  function handleCancel() {
    if (is2FA) {
      sessionStorage.removeItem('totp_pending');
    }
    goto('/signin');
  }
</script>

<div class={cn('min-h-screen flex', className)} {...restProps}>
  <!-- Left Side - Animated Background with Features -->
  <div class="flex-1 relative hidden lg:flex flex-col overflow-hidden">
    <!-- Animated Background Container -->
    <div class="absolute inset-0 rounded-r-3xl overflow-hidden">
      <!-- Gradient SVG Background as Image -->
      <img src="/Gradientsv2.svg" alt="" class="gradient-svg" />
    </div>

    <!-- Logo Overlay -->
    <div class="absolute inset-0 flex items-center justify-center z-10">
      <div class="text-center text-white">
        <img
          src="/logos/CSF_Logo.png"
          alt="CSFX-Core Logo"
          class="mx-auto mb-4 w-200 h-200 md:w-200 h-200 lg:w-[240px] h-[240px]"
        />

        <h2 class="text-4xl font-bold mb-4 text-shadow-glow">CSFX-Core</h2>
        <div class="space-y-2 text-lg text-shadow-glow">The AI-Ready Business Platform</div>
      </div>
    </div>
  </div>

  <!-- Right Side - OTP Form -->
  <div class="flex-1 flex items-center justify-center p-8 bg-background">
    <div class="w-full max-w-md space-y-8">
      <div class="text-center">
        <h1 class="text-3xl font-bold tracking-tight">
          {is2FA ? 'Zwei-Faktor-Authentifizierung' : 'OTP Verifizierung'}
        </h1>
        <p class="text-muted-foreground mt-2">
          {is2FA
            ? 'Geben Sie den 6-stelligen Code aus Ihrer Authenticator-App ein'
            : 'Geben Sie den erhaltenen Code ein, um fortzufahren.'}
        </p>
      </div>

      <form onsubmit={handleSubmit} class="space-y-6">
        {#if errorMessage}
          <Alert variant="destructive">
            <AlertDescription>{errorMessage}</AlertDescription>
          </Alert>
        {/if}

        <FieldGroup class="space-y-6">
          <Field>
            <FieldLabel for="{id}-otp" class="text-base font-semibold mb-3">
              {is2FA ? '2FA-Code' : 'OTP Code'} eingeben
            </FieldLabel>

            <div class="flex justify-center">
              <InputOTP.Root bind:value={otp} maxlength={6} id="{id}-otp" class="gap-3">
                {#snippet children({ cells })}
                  <InputOTP.Group class="gap-2">
                    {#each cells.slice(0, 3) as cell (cell)}
                      <InputOTP.Slot {cell} class="w-12 h-14 text-xl font-bold" />
                    {/each}
                  </InputOTP.Group>
                  <InputOTP.Separator class="text-2xl font-bold text-muted-foreground" />
                  <InputOTP.Group class="gap-2">
                    {#each cells.slice(3, 6) as cell (cell)}
                      <InputOTP.Slot {cell} class="w-12 h-14 text-xl font-bold" />
                    {/each}
                  </InputOTP.Group>
                {/snippet}
              </InputOTP.Root>
            </div>

            <FieldDescription class="text-center mt-3">6-stelliger Code</FieldDescription>
          </Field>

          <div class="flex gap-3">
            <Button
              type="submit"
              class="flex-1 h-12 text-base font-semibold shadow-lg hover:shadow-xl transition-all"
              disabled={isLoading || otp.length !== 6}
            >
              {#if isLoading}
                <div class="animate-spin rounded-full h-5 w-5 border-b-2 border-current mr-2"></div>
              {/if}
              {isLoading ? 'Verifizieren...' : 'Verifizieren'}
            </Button>

            {#if is2FA}
              <Button
                type="button"
                variant="outline"
                class="h-12"
                onclick={handleCancel}
                disabled={isLoading}
              >
                Abbrechen
              </Button>
            {/if}
          </div>
        </FieldGroup>
      </form>

      {#if is2FA}
        <div class="text-center text-sm text-muted-foreground">
          <p>Kein Zugriff auf Ihre Authenticator-App?</p>
          <button onclick={handleCancel} class="text-primary hover:underline mt-1">
            Zurück zum Login
          </button>
        </div>
      {/if}
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
