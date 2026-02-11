$ErrorActionPreference = "Stop"

function Invoke-GitChecked {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,
        [string]$Context
    )

    & git @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "git command failed ($Context): git $($Arguments -join ' ')"
    }
}

function Sync-Repo {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [switch]$UseHttp11
    )

    if (-not (Test-Path $Path -PathType Container)) {
        throw "Repo path not found: $Path"
    }

    if (-not (Test-Path (Join-Path $Path ".git") -PathType Container)) {
        throw "Not a git repository: $Path"
    }

    $gitArgs = @("-C", $Path)
    if ($UseHttp11) {
        $gitArgs += @("-c", "http.version=HTTP/1.1")
    }

    Write-Host "[sync] $Path"

    try {
        Invoke-GitChecked -Arguments ($gitArgs + @("fetch", "--all", "--tags", "--prune")) -Context "$Path fetch"
    }
    catch {
        if (-not $UseHttp11) {
            Write-Warning "[sync] fetch failed for $Path, retrying with HTTP/1.1"
            Invoke-GitChecked -Arguments ($gitArgs + @("-c", "http.version=HTTP/1.1", "fetch", "--all", "--tags", "--prune")) -Context "$Path fetch http1.1"
        }
        else {
            throw
        }
    }

    try {
        Invoke-GitChecked -Arguments ($gitArgs + @("pull", "--ff-only")) -Context "$Path pull"
    }
    catch {
        if (-not $UseHttp11) {
            Write-Warning "[sync] pull failed for $Path, retrying with HTTP/1.1"
            Invoke-GitChecked -Arguments ($gitArgs + @("-c", "http.version=HTTP/1.1", "pull", "--ff-only")) -Context "$Path pull http1.1"
        }
        else {
            throw
        }
    }

    $branch = (& git @gitArgs rev-parse --abbrev-ref HEAD).Trim()
    $head = (& git @gitArgs rev-parse --short HEAD).Trim()
    Write-Host "  branch=$branch head=$head"
}

try {
    Sync-Repo -Path "reference/VCPToolBox"
    Sync-Repo -Path "reference/VCPChat"
    Write-Host "[sync] done"
}
catch {
    Write-Error $_
    exit 1
}
