param(
    [Parameter(Mandatory = $true)]
    [string]$ArtifactPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $ArtifactPath)) {
    Write-Error "Artifact not found: $ArtifactPath"
}

$resolvedArtifactPath = (Resolve-Path -LiteralPath $ArtifactPath).Path
$mode = if (![string]::IsNullOrWhiteSpace($env:ARKTOOLS_WINDOWS_SIGNING_MODE)) {
    $env:ARKTOOLS_WINDOWS_SIGNING_MODE
} else {
    $env:BLUR_WINDOWS_SIGNING_MODE
}

if ([string]::IsNullOrWhiteSpace($mode) -or $mode -eq "none") {
    Write-Host "Skipping Windows signing for $resolvedArtifactPath"
    exit 0
}

if ($mode -ne "trusted-signing-cli") {
    Write-Error "Unsupported Windows signing mode '$mode'. Supported values: none, trusted-signing-cli."
}

# Resolve variables with fallback support
$endpoint = if (![string]::IsNullOrWhiteSpace($env:ARKTOOLS_TRUSTED_SIGNING_ENDPOINT)) { $env:ARKTOOLS_TRUSTED_SIGNING_ENDPOINT } else { $env:BLUR_TRUSTED_SIGNING_ENDPOINT }
$account = if (![string]::IsNullOrWhiteSpace($env:ARKTOOLS_TRUSTED_SIGNING_ACCOUNT)) { $env:ARKTOOLS_TRUSTED_SIGNING_ACCOUNT } else { $env:BLUR_TRUSTED_SIGNING_ACCOUNT }
$profile = if (![string]::IsNullOrWhiteSpace($env:ARKTOOLS_TRUSTED_SIGNING_PROFILE)) { $env:ARKTOOLS_TRUSTED_SIGNING_PROFILE } else { $env:BLUR_TRUSTED_SIGNING_PROFILE }
$description = if (![string]::IsNullOrWhiteSpace($env:ARKTOOLS_TRUSTED_SIGNING_DESCRIPTION)) {
    $env:ARKTOOLS_TRUSTED_SIGNING_DESCRIPTION
} elseif (![string]::IsNullOrWhiteSpace($env:BLUR_TRUSTED_SIGNING_DESCRIPTION)) {
    $env:BLUR_TRUSTED_SIGNING_DESCRIPTION
} else {
    "ArkTools"
}

$missingEnvVars = @()
if ([string]::IsNullOrWhiteSpace($endpoint)) { $missingEnvVars += "ARKTOOLS_TRUSTED_SIGNING_ENDPOINT" }
if ([string]::IsNullOrWhiteSpace($account)) { $missingEnvVars += "ARKTOOLS_TRUSTED_SIGNING_ACCOUNT" }
if ([string]::IsNullOrWhiteSpace($profile)) { $missingEnvVars += "ARKTOOLS_TRUSTED_SIGNING_PROFILE" }
if ([string]::IsNullOrWhiteSpace($env:AZURE_CLIENT_ID)) { $missingEnvVars += "AZURE_CLIENT_ID" }
if ([string]::IsNullOrWhiteSpace($env:AZURE_CLIENT_SECRET)) { $missingEnvVars += "AZURE_CLIENT_SECRET" }
if ([string]::IsNullOrWhiteSpace($env:AZURE_TENANT_ID)) { $missingEnvVars += "AZURE_TENANT_ID" }

if ($missingEnvVars.Count -gt 0) {
    Write-Error ("Missing required environment variables for trusted-signing-cli: " + ($missingEnvVars -join ", "))
}

& trusted-signing-cli `
    -e $endpoint `
    -a $account `
    -c $profile `
    -d $description `
    $resolvedArtifactPath

exit $LASTEXITCODE
