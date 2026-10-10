# rogg installer for Windows (docs/superpowers/specs/2026-10-10-rogg-install-script-design.md §9):
#   irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1 | iex
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/rogg-log/rogg/main/install.ps1))) -Uninstall
# It asks before changing anything, checks downloads against SHA256SUMS, and never touches settings or data.
# Everything runs inside the script block below: nothing leaks into your PowerShell session, and a cut-off download
# doesn't parse, so it runs nothing. $env:ROGG_INSTALL_BASE overrides the release folder URL.
param([switch]$Uninstall)

& {
  $ErrorActionPreference = 'Stop'
  # Windows PowerShell 5.1's progress bar slows Invoke-WebRequest by an order of magnitude.
  $ProgressPreference = 'SilentlyContinue'
  $Base = if ($env:ROGG_INSTALL_BASE) { $env:ROGG_INSTALL_BASE.TrimEnd('/') } else { 'https://github.com/rogg-log/rogg/releases/latest/download' }
  $Archive = 'rogg-windows-x86_64.zip'
  $DefaultDir = Join-Path $env:LOCALAPPDATA 'Programs\rogg'
  $Shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'rogg.lnk'
  $Files = @('rogg.exe', 'README.md', 'Inter-OFL.txt', 'JetBrainsMono-OFL.txt')

  function Say([string]$Text) { Write-Host $Text }
  function Fail([string]$Text) { throw "rogg install: $Text" }

  function Ask([string]$Prompt, [string]$Default) {
    $reply = Read-Host $Prompt
    if ([string]::IsNullOrWhiteSpace($reply)) { return $Default }
    return $reply.Trim()
  }

  function YesNo([string]$Question, [bool]$Default) {
    $hint = if ($Default) { '[Y/n]' } else { '[y/N]' }
    $reply = Read-Host "$Question $hint"
    if ([string]::IsNullOrWhiteSpace($reply)) { return $Default }
    return $reply.Trim().ToLower().StartsWith('y')
  }

  # "0.1.0 6", "0.1.0 local", "unknown", or $null when there is no rogg.exe.
  function Get-InstalledVersion([string]$Exe) {
    if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) { return $null }
    $text = (Get-Item -LiteralPath $Exe).VersionInfo.ProductVersion
    if ($text -match '^rogg (\d+\.\d+\.\d+) \(build (\d+)') { return "$($Matches[1]) $($Matches[2])" }
    if ($text -match '^rogg (\d+\.\d+\.\d+) \(local') { return "$($Matches[1]) local" }
    return 'unknown'
  }

  function Describe($Version) {
    if ($null -eq $Version) { return 'not installed' }
    if ($Version -eq 'unknown') { return 'rogg, unknown version' }
    $parts = $Version.Split(' ')
    if ($parts[1] -eq 'local') { return "rogg $($parts[0]), a local build" }
    return "rogg $($parts[0]) build $($parts[1])"
  }

  function Resolve-Folder([string]$Answer) {
    $p = [Environment]::ExpandEnvironmentVariables($Answer)
    if ($p.StartsWith('~')) { $p = Join-Path $env:USERPROFILE $p.Substring(1).TrimStart('\', '/') }
    if ($p -notmatch '^([A-Za-z]:\\|\\\\)') { Fail "give an absolute folder such as C:\Tools\rogg (not `"$Answer`")" }
    if ($p -match '["$]') { Fail 'the folder name cannot contain " or $' }
    # A bare drive would become a drive-relative "C:" entry on PATH.
    if ($p -match '^[A-Za-z]:\\*$') { Fail "choose a folder, not the whole drive $p" }
    return $p.TrimEnd('\')
  }

  # Downloads <name> from the release; a failure says which file and where from.
  function Get-ReleaseFile([string]$Name, [string]$OutFile) {
    try { Invoke-WebRequest -UseBasicParsing "$Base/$Name" -OutFile $OutFile }
    catch { Fail "could not download $Name from $Base ($($_.Exception.Message))" }
  }

  # The user PATH as stored (keeping %VAR% entries unexpanded), and writing it back as an expandable string.
  function Get-UserPath { return [string](Get-Item 'HKCU:\Environment').GetValue('Path', '', 'DoNotExpandEnvironmentNames') }
  function Set-UserPath([string]$Value) {
    Set-ItemProperty -Path 'HKCU:\Environment' -Name Path -Value $Value -Type ExpandString
    # Setting and clearing a variable through .NET broadcasts the change, so new terminals see the new PATH.
    [Environment]::SetEnvironmentVariable('ROGG_INSTALL_REFRESH', '1', 'User')
    [Environment]::SetEnvironmentVariable('ROGG_INSTALL_REFRESH', $null, 'User')
  }
  function Test-OnPath([string]$Dir, [string]$PathValue) {
    $want = $Dir.TrimEnd('\')
    foreach ($entry in $PathValue.Split(';')) {
      if ([Environment]::ExpandEnvironmentVariables($entry).TrimEnd('\') -eq $want) { return $true }
    }
    return $false
  }

  function Add-ToPath([string]$Dir) {
    $user = Get-UserPath
    if (Test-OnPath $Dir $user) { return }
    if (YesNo "$Dir is not on your PATH. Add it?" $true) {
      $new = if ($user.Trim(';')) { "$($user.TrimEnd(';'));$Dir" } else { $Dir }
      Set-UserPath $new
      Say 'Added to your PATH. Open a new terminal to use rogg.'
    }
  }

  function Add-Shortcut([string]$Exe) {
    $shell = New-Object -ComObject WScript.Shell
    if ((Test-Path -LiteralPath $Shortcut) -and ($shell.CreateShortcut($Shortcut).TargetPath -eq $Exe)) { return }
    if (-not (YesNo 'Add rogg to the Start menu?' $true)) { return }
    $link = $shell.CreateShortcut($Shortcut)
    $link.TargetPath = $Exe
    $link.WorkingDirectory = Split-Path -Parent $Exe
    $link.IconLocation = "$Exe,0"
    $link.Description = 'rogg log viewer'
    $link.Save()
    Say 'Added rogg to the Start menu.'
  }

  function Show-OtherCopy([string]$Exe) {
    $cmd = Get-Command rogg -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($cmd -and ($cmd.Path -ne $Exe)) {
      Say "Note: typing rogg runs $($cmd.Path) ($(Describe (Get-InstalledVersion $cmd.Path))), which comes first on PATH."
    }
  }

  function Remove-OldExes([string]$Dir) {
    Get-ChildItem -LiteralPath $Dir -Filter 'rogg.exe*.old' -ErrorAction SilentlyContinue |
      Remove-Item -Force -ErrorAction SilentlyContinue
  }

  function Install-Rogg {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    if (-not [Environment]::Is64BitOperatingSystem) { Fail 'rogg needs 64-bit Windows' }
    $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    if ($arch -eq 'ARM64') { Say 'Windows on ARM: installing the x64 build, which runs under emulation.' }

    try { $manifest = Invoke-RestMethod -UseBasicParsing "$Base/manifest.json" }
    catch { Fail "could not download manifest.json from $Base ($($_.Exception.Message))" }
    $version = [string]$manifest.version
    $buildText = [string]$manifest.build
    if ($version -notmatch '^\d+\.\d+\.\d+$' -or $buildText -notmatch '^\d+$') { Fail "the release manifest at $Base could not be read" }
    $build = [int]$buildText
    Say "rogg installer - latest release: rogg $version build $build"

    $dir = Resolve-Folder (Ask "Install to [$DefaultDir]" $DefaultDir)
    $exe = Join-Path $dir 'rogg.exe'
    if (Test-Path -LiteralPath $exe -PathType Container) { Fail "$exe is a folder; choose another place" }
    $current = Get-InstalledVersion $exe
    Say "Installed:  $(Describe $current) ($dir)"
    Say "Installing: rogg $version build $build"
    if ($current -match '^\S+ (\d+)$') {
      $have = [int]$Matches[1]
      if ($have -eq $build) { $go = YesNo 'Already up to date. Reinstall?' $false }
      elseif ($have -gt $build) { $go = YesNo 'The installed build is newer than the latest release. Downgrade?' $false }
      else { $go = YesNo 'Install?' $true }
    } else { $go = YesNo 'Install?' $true }
    if (-not $go) { Say 'Nothing changed.'; return }

    $tmp = Join-Path ([IO.Path]::GetTempPath()) ('rogg-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
      $zip = Join-Path $tmp $Archive
      $sums = Join-Path $tmp 'SHA256SUMS'
      Get-ReleaseFile $Archive $zip
      Get-ReleaseFile 'SHA256SUMS' $sums
      $want = $null
      foreach ($line in Get-Content -LiteralPath $sums) {
        $fields = $line -split '\s+', 2
        if ($fields.Count -eq 2 -and $fields[1].Trim() -eq $Archive) { $want = $fields[0].ToLower() }
      }
      if (-not $want) { Fail "$Archive is not listed in SHA256SUMS; nothing was changed" }
      $got = (Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash.ToLower()
      if ($got -ne $want) { Fail "checksum mismatch for $Archive; nothing was changed" }
      $unpacked = Join-Path $tmp 'x'
      Expand-Archive -LiteralPath $zip -DestinationPath $unpacked

      New-Item -ItemType Directory -Force -Path $dir | Out-Null
      Remove-OldExes $dir
      # The new exe is copied next to the old one first, so a failed copy (a full disk) leaves the old one untouched.
      $new = Join-Path $dir 'rogg.exe.new'
      Remove-Item -LiteralPath $new -Force -ErrorAction SilentlyContinue
      Copy-Item -LiteralPath (Join-Path $unpacked 'rogg.exe') -Destination $new -Force
      # A running rogg.exe can't be overwritten but can be renamed; the renamed file goes on the next run.
      $old = $null
      if (Test-Path -LiteralPath $exe) {
        $old = "$exe.$([guid]::NewGuid().ToString('N').Substring(0, 8)).old"
        Move-Item -LiteralPath $exe -Destination $old
      }
      try {
        Move-Item -LiteralPath $new -Destination $exe
      } catch {
        if ($old) { Move-Item -LiteralPath $old -Destination $exe }
        throw
      }
      foreach ($f in $Files) {
        $from = Join-Path $unpacked $f
        if ($f -ne 'rogg.exe' -and (Test-Path -LiteralPath $from)) { Copy-Item -LiteralPath $from -Destination $dir -Force }
      }
      # Only rogg's own files; other files in a folder the user chose keep their download mark.
      foreach ($f in $Files) {
        $p = Join-Path $dir $f
        if (Test-Path -LiteralPath $p) { Unblock-File -LiteralPath $p -ErrorAction SilentlyContinue }
      }
      Remove-OldExes $dir
      Say "Installed rogg $version build $build to $dir."
    } finally {
      Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
    Add-ToPath $dir
    Add-Shortcut $exe
    Show-OtherCopy $exe
  }

  function Uninstall-Rogg {
    $exe = Join-Path $DefaultDir 'rogg.exe'
    $hasExe = Test-Path -LiteralPath $exe -PathType Leaf
    # Only a Start-menu entry pointing into the default folder is ours to remove.
    $hasLink = $false
    if (Test-Path -LiteralPath $Shortcut) {
      $target = (New-Object -ComObject WScript.Shell).CreateShortcut($Shortcut).TargetPath
      $hasLink = $target.StartsWith($DefaultDir, [StringComparison]::OrdinalIgnoreCase)
      if (-not $hasLink) { Say "The Start menu entry points to $target; it is left alone." }
    }
    if (-not $hasExe -and -not $hasLink) {
      Say "No rogg installation found in $DefaultDir."
    } else {
      if ($hasExe) { Say "Found: $exe - $(Describe (Get-InstalledVersion $exe))" }
      if ($hasLink) { Say "Found: $Shortcut (Start menu)" }
      if (YesNo 'Remove it?' $true) {
        foreach ($f in $Files) { Remove-Item -LiteralPath (Join-Path $DefaultDir $f) -Force -ErrorAction SilentlyContinue }
        Remove-Item -LiteralPath (Join-Path $DefaultDir 'rogg.exe.new') -Force -ErrorAction SilentlyContinue
        Remove-OldExes $DefaultDir
        if ($hasLink) { Remove-Item -LiteralPath $Shortcut -Force -ErrorAction SilentlyContinue }
        if (Test-Path -LiteralPath $exe) {
          Say 'rogg.exe is in use: quit rogg and run the uninstaller again to remove it.'
        } else {
          if ((Test-Path -LiteralPath $DefaultDir) -and -not (Get-ChildItem -LiteralPath $DefaultDir -Force)) {
            Remove-Item -LiteralPath $DefaultDir -Force
          }
          Say 'Removed.'
        }
      }
    }
    $user = Get-UserPath
    if (Test-OnPath $DefaultDir $user) {
      if (YesNo "Remove $DefaultDir from your PATH?" $true) {
        $want = $DefaultDir.TrimEnd('\')
        $kept = @($user.Split(';') | Where-Object { $_ -and ([Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -ne $want) })
        Set-UserPath ($kept -join ';')
      }
    }
    $cmd = Get-Command rogg -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($cmd -and -not $cmd.Path.StartsWith($DefaultDir, [StringComparison]::OrdinalIgnoreCase)) {
      Say "Also on PATH: $($cmd.Path) ($(Describe (Get-InstalledVersion $cmd.Path))). The installer did not put it there; remove it yourself if you want."
    }
    Say 'A rogg installed into another folder you chose has to be removed by hand.'
    Say "Settings and data are kept in $env:APPDATA\rogg and $env:LOCALAPPDATA\rogg\cache; delete them yourself if you want."
  }

  try {
    if ($Uninstall) { Uninstall-Rogg } else { Install-Rogg }
  } catch {
    # No exit: under `irm | iex` it would close the user's PowerShell window.
    Write-Host $_.Exception.Message -ForegroundColor Red
  }
}
