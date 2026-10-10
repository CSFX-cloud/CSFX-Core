export interface ProviderPreset {
    id: string;
    label: string;
    icon: string;
    iconColor?: string;
    slug: string;
    issuerPlaceholder: string;
    scopes: string;
    usernameClaim: string;
    groupsClaim: string;
    hint: string;
}

export const PROVIDER_PRESETS: ProviderPreset[] = [
    {
        id: "authentik",
        label: "Authentik",
        icon: "simple-icons:authentik",
        iconColor: "#fd4b2d",
        slug: "authentik",
        issuerPlaceholder: "https://authentik.example.com/application/o/csfx/",
        scopes: "openid profile email",
        usernameClaim: "preferred_username",
        groupsClaim: "groups",
        hint: "Create an OAuth2/OpenID provider (confidential) and an application in Authentik. The issuer is shown on the provider page as OpenID Configuration Issuer.",
    },
    {
        id: "keycloak",
        label: "Keycloak",
        icon: "simple-icons:keycloak",
        iconColor: "#4d4d4d",
        slug: "keycloak",
        issuerPlaceholder: "https://keycloak.example.com/realms/csfx",
        scopes: "openid profile email",
        usernameClaim: "preferred_username",
        groupsClaim: "groups",
        hint: "Create a confidential client with standard flow. Add a Group Membership mapper named groups to include groups in the ID token.",
    },
    {
        id: "entra",
        label: "Microsoft Entra ID",
        icon: "logos:microsoft-azure",
        slug: "entra",
        issuerPlaceholder: "https://login.microsoftonline.com/<tenant-id>/v2.0",
        scopes: "openid profile email",
        usernameClaim: "preferred_username",
        groupsClaim: "groups",
        hint: "Register a web app, add a client secret and enable the groups claim in the token configuration. Groups arrive as object IDs.",
    },
    {
        id: "okta",
        label: "Okta",
        icon: "logos:okta-icon",
        slug: "okta",
        issuerPlaceholder: "https://your-org.okta.com",
        scopes: "openid profile email groups",
        usernameClaim: "preferred_username",
        groupsClaim: "groups",
        hint: "Create an OIDC web application and add a groups claim to the ID token in the authorization server.",
    },
    {
        id: "auth0",
        label: "Auth0",
        icon: "logos:auth0-icon",
        slug: "auth0",
        issuerPlaceholder: "https://your-tenant.eu.auth0.com/",
        scopes: "openid profile email",
        usernameClaim: "nickname",
        groupsClaim: "groups",
        hint: "Create a regular web application. Groups require an Action that adds them as a custom claim.",
    },
    {
        id: "google",
        label: "Google",
        icon: "logos:google-icon",
        slug: "google",
        issuerPlaceholder: "https://accounts.google.com",
        scopes: "openid profile email",
        usernameClaim: "email",
        groupsClaim: "groups",
        hint: "Google does not send groups. Set a default role so every Google account that may sign in gets access.",
    },
    {
        id: "generic",
        label: "Generic OIDC",
        icon: "mdi:key-variant",
        slug: "",
        issuerPlaceholder: "https://idp.example.com",
        scopes: "openid profile email",
        usernameClaim: "preferred_username",
        groupsClaim: "groups",
        hint: "Any OpenID Connect provider with a discovery document at /.well-known/openid-configuration.",
    },
];
