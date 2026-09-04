; DSH Launcher diagnostic-only derivative of Tauri CLI 2.11.4.
; Source: https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi
; Copyright (c) 2017 - Present Tauri Apps Contributors; MIT, see LICENSE_MIT.
; No install/uninstall sections may execute; logs are opt-in by running this clearly labelled probe.
Unicode true
ManifestDPIAware true
; Add in `dpiAwareness` `PerMonitorV2` to manifest for Windows 10 1607+ (note this should not affect lower versions since they should be able to ignore this and pick up `dpiAware` `true` set by `ManifestDPIAware true`)
; Currently undocumented on NSIS's website but is in the Docs folder of source tree, see
; https://github.com/kichik/nsis/blob/5fc0b87b819a9eec006df4967d08e522ddd651c9/Docs/src/attributes.but#L286-L300
; https://github.com/tauri-apps/tauri/pull/10106
ManifestDPIAwareness PerMonitorV2

!if "{{compression}}" == "none"
  SetCompress off
!else
  ; Set the compression algorithm. We default to LZMA.
  SetCompressor /SOLID "{{compression}}"
!endif

; Keep above !include to stay ahead of any plugin command
; see https://github.com/tauri-apps/tauri/pull/15422#discussion_r3289239624
{{#if signed_plugins_path}}
!addplugindir "{{signed_plugins_path}}"
{{/if}}

!include MUI2.nsh
!include FileFunc.nsh
!include x64.nsh
!include WordFunc.nsh
!include "utils.nsh"
!include "FileAssociation.nsh"
!include "Win\COM.nsh"
!include "Win\Propkey.nsh"
!include "StrFunc.nsh"
${StrCase}
${StrLoc}

{{#if installer_hooks}}
!include "{{installer_hooks}}"
{{/if}}

!define WEBVIEW2APPGUID "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

!define MANUFACTURER "{{manufacturer}}"
!define PRODUCTNAME "{{product_name}}"
!define VERSION "{{version}}"
!define VERSIONWITHBUILD "{{version_with_build}}"
!define HOMEPAGE "{{homepage}}"
!define INSTALLMODE "{{install_mode}}"
!define LICENSE "{{license}}"
!define INSTALLERICON "{{installer_icon}}"
!define SIDEBARIMAGE "{{sidebar_image}}"
!define HEADERIMAGE "{{header_image}}"
!define UNINSTALLERICON "{{uninstaller_icon}}"
!define UNINSTALLERHEADERIMAGE "{{uninstaller_header_image}}"
!define MAINBINARYNAME "{{main_binary_name}}"
!define MAINBINARYSRCPATH "{{main_binary_path}}"
!define BUNDLEID "{{bundle_id}}"
!define COPYRIGHT "{{copyright}}"
!define OUTFILE "{{out_file}}"
!define ARCH "{{arch}}"
!define ADDITIONALPLUGINSPATH "{{additional_plugins_path}}"
!define ALLOWDOWNGRADES "{{allow_downgrades}}"
!define DISPLAYLANGUAGESELECTOR "{{display_language_selector}}"
!define INSTALLWEBVIEW2MODE "{{install_webview2_mode}}"
!define WEBVIEW2INSTALLERARGS "{{webview2_installer_args}}"
!define WEBVIEW2BOOTSTRAPPERPATH "{{webview2_bootstrapper_path}}"
!define WEBVIEW2INSTALLERPATH "{{webview2_installer_path}}"
!define MINIMUMWEBVIEW2VERSION "{{minimum_webview2_version}}"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}"
!define MANUKEY "Software\${MANUFACTURER}"
!define MANUPRODUCTKEY "${MANUKEY}\${PRODUCTNAME}"
!define UNINSTALLERSIGNCOMMAND "{{uninstaller_sign_cmd}}"
!define ESTIMATEDSIZE "{{estimated_size}}"
!define STARTMENUFOLDER "{{start_menu_folder}}"

Var PassiveMode
Var UpdateMode
Var NoShortcutMode
Var WixMode
Var OldMainBinaryName

Name "${PRODUCTNAME}"
BrandingText "${COPYRIGHT}"
OutFile "${OUTFILE}"

; We don't actually use this value as default install path,
; it's just for nsis to append the product name folder in the directory selector
; https://nsis.sourceforge.io/Reference/InstallDir
!define PLACEHOLDER_INSTALL_DIR "placeholder\${PRODUCTNAME}"
InstallDir "${PLACEHOLDER_INSTALL_DIR}"

VIProductVersion "${VERSIONWITHBUILD}"
VIAddVersionKey "ProductName" "${PRODUCTNAME}"
VIAddVersionKey "FileDescription" "${PRODUCTNAME}"
VIAddVersionKey "LegalCopyright" "${COPYRIGHT}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"

# additional plugins
!addplugindir "${ADDITIONALPLUGINSPATH}"

; Uninstaller signing command
!if "${UNINSTALLERSIGNCOMMAND}" != ""
  !uninstfinalize '${UNINSTALLERSIGNCOMMAND}'
!endif

; Handle install mode, `perUser`, `perMachine` or `both`
!if "${INSTALLMODE}" == "perMachine"
  RequestExecutionLevel admin
!endif

!if "${INSTALLMODE}" == "currentUser"
  RequestExecutionLevel user
!endif

!if "${INSTALLMODE}" == "both"
  !define MULTIUSER_MUI
  !define MULTIUSER_INSTALLMODE_INSTDIR "${PRODUCTNAME}"
  !define MULTIUSER_INSTALLMODE_COMMANDLINE
  !if "${ARCH}" == "x64"
    !define MULTIUSER_USE_PROGRAMFILES64
  !else if "${ARCH}" == "arm64"
    !define MULTIUSER_USE_PROGRAMFILES64
  !endif
  !define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_KEY "${UNINSTKEY}"
  !define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_VALUENAME "CurrentUser"
  !define MULTIUSER_INSTALLMODEPAGE_SHOWUSERNAME
  !define MULTIUSER_INSTALLMODE_FUNCTION RestorePreviousInstallLocation
  !define MULTIUSER_EXECUTIONLEVEL Highest
  !include MultiUser.nsh
!endif

; Installer icon
!if "${INSTALLERICON}" != ""
  !define MUI_ICON "${INSTALLERICON}"
!endif

; Installer sidebar image
!if "${SIDEBARIMAGE}" != ""
  !define MUI_WELCOMEFINISHPAGE_BITMAP "${SIDEBARIMAGE}"
!endif

; Enable header images for installer and uninstaller pages when either image is configured.
!if "${HEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE
!else if "${UNINSTALLERHEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE
!endif

; Installer header image
!if "${HEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE_BITMAP "${HEADERIMAGE}"
!endif

; Uninstaller header image
!if "${UNINSTALLERHEADERIMAGE}" != ""
  !define MUI_HEADERIMAGE_UNBITMAP "${UNINSTALLERHEADERIMAGE}"
!endif

; Uninstaller icon
!if "${UNINSTALLERICON}" != ""
  !define MUI_UNICON "${UNINSTALLERICON}"
!endif

; Define registry key to store installer language
!define MUI_LANGDLL_REGISTRY_ROOT "HKCU"
!define MUI_LANGDLL_REGISTRY_KEY "${MANUPRODUCTKEY}"
!define MUI_LANGDLL_REGISTRY_VALUENAME "Installer Language"

; Installer pages, must be ordered as they appear
; 1. Welcome Page
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_WELCOME

; 2. License Page (if defined)
!if "${LICENSE}" != ""
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !insertmacro MUI_PAGE_LICENSE "${LICENSE}"
!endif

; 3. Install mode (if it is set to `both`)
!if "${INSTALLMODE}" == "both"
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !insertmacro MULTIUSER_PAGE_INSTALLMODE
!endif

; 4. Custom page to ask user if he wants to reinstall/uninstall
;    only if a previous installation was detected
Var DiagnosticHandle
Var DiagnosticPath
Var DiagnosticPid
Var ReinstallPageCheck
Page custom PageReinstall PageLeaveReinstall
Function PageReinstall
  FileWriteUTF16LE $DiagnosticHandle "event=PageReinstall-enter$\r$\n"
  ; Uninstall previous WiX installation if exists.
  ;
  ; A WiX installer stores the installation info in registry
  ; using a UUID and so we have to loop through all keys under
  ; `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`
  ; and check if `DisplayName` and `Publisher` keys match ${PRODUCTNAME} and ${MANUFACTURER}
  ;
  ; This has a potential issue that there maybe another installation that matches
  ; our ${PRODUCTNAME} and ${MANUFACTURER} but wasn't installed by our WiX installer,
  ; however, this should be fine since the user will have to confirm the uninstallation
  ; and they can chose to abort it if doesn't make sense.
  StrCpy $0 0
  wix_loop:
    EnumRegKey $1 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall" $0
    StrCmp $1 "" wix_loop_done ; Exit loop if there is no more keys to loop on
    IntOp $0 $0 + 1
    ReadRegStr $R0 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "DisplayName"
    ReadRegStr $R1 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "Publisher"
    StrCmp "$R0$R1" "${PRODUCTNAME}${MANUFACTURER}" 0 wix_loop
    ReadRegStr $R0 HKLM "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1" "UninstallString"
    ${StrCase} $R1 $R0 "L"
    ${StrLoc} $R0 $R1 "msiexec" ">"
    StrCmp $R0 0 0 wix_loop_done
    FileWriteUTF16LE $DiagnosticHandle "event=wix-match; key=$1; uninstall=$R1$\r$\n"
    StrCpy $WixMode 1
    StrCpy $R6 "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$1"
    Goto compare_version
  wix_loop_done:

  ; Check if there is an existing installation, if not, abort the reinstall page
  ReadRegStr $R0 SHCTX "${UNINSTKEY}" ""
  ReadRegStr $R1 SHCTX "${UNINSTKEY}" "UninstallString"
  FileWriteUTF16LE $DiagnosticHandle "event=nsis-detection; default=$R0; uninstall=$R1; key=${UNINSTKEY}$\r$\n"
  ${If} "$R0$R1" == ""
    FileWriteUTF16LE $DiagnosticHandle "decision=no-existing-install$\r$\n"
    Abort
  ${EndIf}

  ; Compare this installar version with the existing installation
  ; and modify the messages presented to the user accordingly
  compare_version:
  StrCpy $R4 "$(older)"
  ${If} $WixMode = 1
    ReadRegStr $R0 HKLM "$R6" "DisplayVersion"
  ${Else}
    ReadRegStr $R0 SHCTX "${UNINSTKEY}" "DisplayVersion"
  ${EndIf}
  ${IfThen} $R0 == "" ${|} StrCpy $R4 "$(unknown)" ${|}

  FileWriteUTF16LE $DiagnosticHandle "event=compare-version; wix=$WixMode; wixKey=$R6; oldVersion=$R0; description=$R4; newVersion=${VERSION}$\r$\n"
  nsis_tauri_utils::SemverCompare "${VERSION}" $R0
  Pop $R0
  FileWriteUTF16LE $DiagnosticHandle "decision=maintenance-page; semverResult=$R0$\r$\n"
  ; Reinstalling the same version
  ${If} $R0 = 0
    StrCpy $R1 "$(alreadyInstalledLong)"
    StrCpy $R2 "$(addOrReinstall)"
    StrCpy $R3 "$(uninstallApp)"
    !insertmacro MUI_HEADER_TEXT "$(alreadyInstalled)" "$(chooseMaintenanceOption)"
  ; Upgrading
  ${ElseIf} $R0 = 1
    StrCpy $R1 "$(olderOrUnknownVersionInstalled)"
    StrCpy $R2 "$(uninstallBeforeInstalling)"
    StrCpy $R3 "$(dontUninstall)"
    !insertmacro MUI_HEADER_TEXT "$(alreadyInstalled)" "$(choowHowToInstall)"
  ; Downgrading
  ${ElseIf} $R0 = -1
    StrCpy $R1 "$(newerVersionInstalled)"
    StrCpy $R2 "$(uninstallBeforeInstalling)"
    !if "${ALLOWDOWNGRADES}" == "true"
      StrCpy $R3 "$(dontUninstall)"
    !else
      StrCpy $R3 "$(dontUninstallDowngrade)"
    !endif
    !insertmacro MUI_HEADER_TEXT "$(alreadyInstalled)" "$(choowHowToInstall)"
  ${Else}
    Abort
  ${EndIf}

  ; Skip showing the page if passive
  ;
  ; Note that we don't call this earlier at the begining
  ; of this function because we need to populate some variables
  ; related to current installed version if detected and whether
  ; we are downgrading or not.
  ${If} $PassiveMode = 1
    Call PageLeaveReinstall
  ${Else}
    nsDialogs::Create 1018
    Pop $R4
    ${IfThen} $(^RTL) = 1 ${|} nsDialogs::SetRTL $(^RTL) ${|}

    ${NSD_CreateLabel} 0 0 100% 24u $R1
    Pop $R1

    ${NSD_CreateRadioButton} 30u 50u -30u 8u $R2
    Pop $R2
    ${NSD_OnClick} $R2 PageReinstallUpdateSelection

    ${NSD_CreateRadioButton} 30u 70u -30u 8u $R3
    Pop $R3
    ; Disable this radio button if downgrading and downgrades are disabled
    !if "${ALLOWDOWNGRADES}" == "false"
      ${IfThen} $R0 = -1 ${|} EnableWindow $R3 0 ${|}
    !endif
    ${NSD_OnClick} $R3 PageReinstallUpdateSelection

    ; Check the first radio button if this the first time
    ; we enter this page or if the second button wasn't
    ; selected the last time we were on this page
    ${If} $ReinstallPageCheck <> 2
      SendMessage $R2 ${BM_SETCHECK} ${BST_CHECKED} 0
    ${Else}
      SendMessage $R3 ${BM_SETCHECK} ${BST_CHECKED} 0
    ${EndIf}

    ${NSD_SetFocus} $R2
    nsDialogs::Show
  ${EndIf}
FunctionEnd
Function PageReinstallUpdateSelection
  ${NSD_GetState} $R2 $R1
  ${If} $R1 == ${BST_CHECKED}
    StrCpy $ReinstallPageCheck 1
  ${Else}
    StrCpy $ReinstallPageCheck 2
  ${EndIf}
FunctionEnd
Function PageLeaveReinstall
  ${NSD_GetState} $R2 $R1

  ; If migrating from Wix, always uninstall
  ${If} $WixMode = 1
    Goto reinst_uninstall
  ${EndIf}

  ; In update mode, always proceeds without uninstalling
  ${If} $UpdateMode = 1
    Goto reinst_done
  ${EndIf}

  ; $R0 holds whether same(0)/upgrading(1)/downgrading(-1) version
  ; $R1 holds the radio buttons state:
  ;   1 => first choice was selected
  ;   0 => second choice was selected
  ${If} $R0 = 0 ; Same version, proceed
    ${If} $R1 = 1              ; User chose to add/reinstall
      Goto reinst_done
    ${Else}                    ; User chose to uninstall
      Goto reinst_uninstall
    ${EndIf}
  ${ElseIf} $R0 = 1 ; Upgrading
    ${If} $R1 = 1              ; User chose to uninstall
      Goto reinst_uninstall
    ${Else}
      Goto reinst_done         ; User chose NOT to uninstall
    ${EndIf}
  ${ElseIf} $R0 = -1 ; Downgrading
    ${If} $R1 = 1              ; User chose to uninstall
      Goto reinst_uninstall
    ${Else}
      Goto reinst_done         ; User chose NOT to uninstall
    ${EndIf}
  ${EndIf}

  reinst_uninstall:
    ; Never execute a registry-provided command in this diagnostic-only build.
    Push $R1
    Push $4
    ${If} $WixMode = 1
      ReadRegStr $R1 HKLM "$R6" "UninstallString"
      FileWriteUTF16LE $DiagnosticHandle "action=uninstall-blocked; mode=WiX; command=$R1$\r$\n"
    ${Else}
      ReadRegStr $4 SHCTX "${MANUPRODUCTKEY}" ""
      ReadRegStr $R1 SHCTX "${UNINSTKEY}" "UninstallString"
      FileWriteUTF16LE $DiagnosticHandle "action=uninstall-blocked; mode=NSIS; command=$R1; installDir=$4$\r$\n"
    ${EndIf}
    Pop $4
    Pop $R1
    Abort
  reinst_done:
FunctionEnd

; 5. Choose install directory page
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_DIRECTORY

; 6. Start menu shortcut page
Var AppStartMenuFolder
!if "${STARTMENUFOLDER}" != ""
  !define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
  !define MUI_STARTMENUPAGE_DEFAULTFOLDER "${STARTMENUFOLDER}"
!else
  !define MUI_PAGE_CUSTOMFUNCTION_PRE Skip
!endif
!insertmacro MUI_PAGE_STARTMENU Application $AppStartMenuFolder

; 7. Installation page
!insertmacro MUI_PAGE_INSTFILES

; 8. Finish page
;
; Don't auto jump to finish page after installation page,
; because the installation page has useful info that can be used debug any issues with the installer.
!define MUI_FINISHPAGE_NOAUTOCLOSE
; Use show readme button in the finish page as a button create a desktop shortcut
!define MUI_FINISHPAGE_SHOWREADME
!define MUI_FINISHPAGE_SHOWREADME_TEXT "$(createDesktop)"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION CreateOrUpdateDesktopShortcut
; Show run app after installation.
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_FUNCTION RunMainBinary
!define MUI_PAGE_CUSTOMFUNCTION_PRE SkipIfPassive
!insertmacro MUI_PAGE_FINISH

Function RunMainBinary
  Abort
FunctionEnd

; Uninstaller Pages
; 1. Confirm uninstall page
Var DeleteAppDataCheckbox
Var DeleteAppDataCheckboxState
!define /ifndef WS_EX_LAYOUTRTL         0x00400000
!define MUI_PAGE_CUSTOMFUNCTION_SHOW un.ConfirmShow
Function un.ConfirmShow ; Add add a `Delete app data` check box
  ; $1 inner dialog HWND
  ; $2 window DPI
  ; $3 style
  ; $4 x
  ; $5 y
  ; $6 width
  ; $7 height
  FindWindow $1 "#32770" "" $HWNDPARENT ; Find inner dialog
  System::Call "user32::GetDpiForWindow(p r1) i .r2"
  ${If} $(^RTL) = 1
    StrCpy $3 "${__NSD_CheckBox_EXSTYLE} | ${WS_EX_LAYOUTRTL}"
    IntOp $4 50 * $2
  ${Else}
    StrCpy $3 "${__NSD_CheckBox_EXSTYLE}"
    IntOp $4 0 * $2
  ${EndIf}
  IntOp $5 100 * $2
  IntOp $6 400 * $2
  IntOp $7 25 * $2
  IntOp $4 $4 / 96
  IntOp $5 $5 / 96
  IntOp $6 $6 / 96
  IntOp $7 $7 / 96
  System::Call 'user32::CreateWindowEx(i r3, w "${__NSD_CheckBox_CLASS}", w "$(deleteAppData)", i ${__NSD_CheckBox_STYLE}, i r4, i r5, i r6, i r7, p r1, i0, i0, i0) i .s'
  Pop $DeleteAppDataCheckbox
  SendMessage $HWNDPARENT ${WM_GETFONT} 0 0 $1
  SendMessage $DeleteAppDataCheckbox ${WM_SETFONT} $1 1
FunctionEnd
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE un.ConfirmLeave
Function un.ConfirmLeave
  SendMessage $DeleteAppDataCheckbox ${BM_GETCHECK} 0 0 $DeleteAppDataCheckboxState
FunctionEnd
!define MUI_PAGE_CUSTOMFUNCTION_PRE un.SkipIfPassive
!insertmacro MUI_UNPAGE_CONFIRM

; 2. Uninstalling Page
!insertmacro MUI_UNPAGE_INSTFILES

;Languages
{{#each languages}}
!insertmacro MUI_LANGUAGE "{{this}}"
{{/each}}
!insertmacro MUI_RESERVEFILE_LANGDLL
{{#each language_files}}
  !include "{{this}}"
{{/each}}

Function .onInit
  Call DiagnosticInit
  ${GetOptions} $CMDLINE "/P" $PassiveMode
  ${IfNot} ${Errors}
    StrCpy $PassiveMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/NS" $NoShortcutMode
  ${IfNot} ${Errors}
    StrCpy $NoShortcutMode 1
  ${EndIf}

  ${GetOptions} $CMDLINE "/UPDATE" $UpdateMode
  ${IfNot} ${Errors}
    StrCpy $UpdateMode 1
  ${EndIf}

  !if "${DISPLAYLANGUAGESELECTOR}" == "true"
    !insertmacro MUI_LANGDLL_DISPLAY
  !endif

  !insertmacro SetContext
  ; Read-only cross-view evidence, using dedicated pushed scratch registers.
  Push $R0
  Push $R1
  SetRegView 32
  ClearErrors
  ReadRegStr $R0 HKCU "${UNINSTKEY}" ""
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKCU/32/uninstallDefault; status=$R1; value=$R0$\r$\n"
  SetRegView 32
  ClearErrors
  ReadRegStr $R0 HKCU "${UNINSTKEY}" "DisplayVersion"
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKCU/32/displayVersion; status=$R1; value=$R0$\r$\n"
  SetRegView 32
  ClearErrors
  ReadRegStr $R0 HKCU "${UNINSTKEY}" "UninstallString"
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKCU/32/uninstallString; status=$R1; value=$R0$\r$\n"
  SetRegView 32
  ClearErrors
  ReadRegStr $R0 HKCU "${MANUPRODUCTKEY}" ""
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKCU/32/installLocation; status=$R1; value=$R0$\r$\n"
  SetRegView 32
  ClearErrors
  ReadRegStr $R0 HKLM "${UNINSTKEY}" ""
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKLM/32/uninstallDefault; status=$R1; value=$R0$\r$\n"
  SetRegView 32
  ClearErrors
  ReadRegStr $R0 HKLM "${UNINSTKEY}" "DisplayVersion"
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKLM/32/displayVersion; status=$R1; value=$R0$\r$\n"
  SetRegView 32
  ClearErrors
  ReadRegStr $R0 HKLM "${UNINSTKEY}" "UninstallString"
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKLM/32/uninstallString; status=$R1; value=$R0$\r$\n"
  SetRegView 32
  ClearErrors
  ReadRegStr $R0 HKLM "${MANUPRODUCTKEY}" ""
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKLM/32/installLocation; status=$R1; value=$R0$\r$\n"
  SetRegView 64
  ClearErrors
  ReadRegStr $R0 HKCU "${UNINSTKEY}" ""
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKCU/64/uninstallDefault; status=$R1; value=$R0$\r$\n"
  SetRegView 64
  ClearErrors
  ReadRegStr $R0 HKCU "${UNINSTKEY}" "DisplayVersion"
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKCU/64/displayVersion; status=$R1; value=$R0$\r$\n"
  SetRegView 64
  ClearErrors
  ReadRegStr $R0 HKCU "${UNINSTKEY}" "UninstallString"
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKCU/64/uninstallString; status=$R1; value=$R0$\r$\n"
  SetRegView 64
  ClearErrors
  ReadRegStr $R0 HKCU "${MANUPRODUCTKEY}" ""
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKCU/64/installLocation; status=$R1; value=$R0$\r$\n"
  SetRegView 64
  ClearErrors
  ReadRegStr $R0 HKLM "${UNINSTKEY}" ""
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKLM/64/uninstallDefault; status=$R1; value=$R0$\r$\n"
  SetRegView 64
  ClearErrors
  ReadRegStr $R0 HKLM "${UNINSTKEY}" "DisplayVersion"
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKLM/64/displayVersion; status=$R1; value=$R0$\r$\n"
  SetRegView 64
  ClearErrors
  ReadRegStr $R0 HKLM "${UNINSTKEY}" "UninstallString"
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKLM/64/uninstallString; status=$R1; value=$R0$\r$\n"
  SetRegView 64
  ClearErrors
  ReadRegStr $R0 HKLM "${MANUPRODUCTKEY}" ""
  StrCpy $R1 "ok"
  ${IfThen} ${Errors} ${|} StrCpy $R1 "read-error-or-missing" ${|}
  FileWriteUTF16LE $DiagnosticHandle "observe=HKLM/64/installLocation; status=$R1; value=$R0$\r$\n"
  Pop $R1
  Pop $R0
  !insertmacro SetContext

  ${If} $INSTDIR == "${PLACEHOLDER_INSTALL_DIR}"
    ; Set default install location
    !if "${INSTALLMODE}" == "perMachine"
      ${If} ${RunningX64}
        !if "${ARCH}" == "x64"
          StrCpy $INSTDIR "$PROGRAMFILES64\${PRODUCTNAME}"
        !else if "${ARCH}" == "arm64"
          StrCpy $INSTDIR "$PROGRAMFILES64\${PRODUCTNAME}"
        !else
          StrCpy $INSTDIR "$PROGRAMFILES\${PRODUCTNAME}"
        !endif
      ${Else}
        StrCpy $INSTDIR "$PROGRAMFILES\${PRODUCTNAME}"
      ${EndIf}
    !else if "${INSTALLMODE}" == "currentUser"
      StrCpy $INSTDIR "$LOCALAPPDATA\${PRODUCTNAME}"
    !endif

    Call RestorePreviousInstallLocation
  ${EndIf}


  !if "${INSTALLMODE}" == "both"
    !insertmacro MULTIUSER_INIT
  !endif
FunctionEnd


Section EarlyChecks
  FileWriteUTF16LE $DiagnosticHandle "action=install-blocked; section=EarlyChecks$\r$\n"
  SetErrorLevel 0
  Quit
SectionEnd

Section WebView2
  FileWriteUTF16LE $DiagnosticHandle "action=install-blocked; section=WebView2$\r$\n"
  SetErrorLevel 0
  Quit
SectionEnd

Section Install
  FileWriteUTF16LE $DiagnosticHandle "action=install-blocked; section=Install$\r$\n"
  SetErrorLevel 0
  Quit
SectionEnd

Function .onInstSuccess
  Abort
FunctionEnd

Function un.onInit
  Abort
FunctionEnd

Section Uninstall
  Abort
SectionEnd

Function RestorePreviousInstallLocation
  ReadRegStr $4 SHCTX "${MANUPRODUCTKEY}" ""
  StrCmp $4 "" +2 0
    StrCpy $INSTDIR $4
FunctionEnd

Function Skip
  Abort
FunctionEnd

Function SkipIfPassive
  ${IfThen} $PassiveMode = 1  ${|} Abort ${|}
FunctionEnd
Function un.SkipIfPassive
  ${IfThen} $PassiveMode = 1  ${|} Abort ${|}
FunctionEnd

Function CreateOrUpdateStartMenuShortcut
  Abort
FunctionEnd

Function CreateOrUpdateDesktopShortcut
  Abort
FunctionEnd

Function DiagnosticInit
  System::Call 'kernel32::GetCurrentProcessId() i .s'
  Pop $DiagnosticPid
  StrCpy $DiagnosticPath "$EXEDIR\installer-diagnostic-$DiagnosticPid.log"
  IfFileExists "$DiagnosticPath" diag_open_failed
  ClearErrors
  FileOpen $DiagnosticHandle "$DiagnosticPath" w
  IfErrors diag_open_failed
  FileWriteWord $DiagnosticHandle 0xFEFF
  FileWriteUTF16LE $DiagnosticHandle "format=dsh-installer-diagnostic-v1$\r$\n"
  FileWriteUTF16LE $DiagnosticHandle "diagnosticOnly=true; no-install=true; no-uninstall=true$\r$\n"
  FileWriteUTF16LE $DiagnosticHandle "product=${PRODUCTNAME}; version=${VERSION}; mode=${INSTALLMODE}; pid=$DiagnosticPid$\r$\n"
  Push $R0
  UserInfo::GetName
  Pop $R0
  FileWriteUTF16LE $DiagnosticHandle "account=$R0$\r$\n"
  UserInfo::GetAccountType
  Pop $R0
  FileWriteUTF16LE $DiagnosticHandle "accountType=$R0; registryContext=HKCU; expectedRegistryView=64$\r$\n"
  Pop $R0
  Return
  diag_open_failed:
    MessageBox MB_ICONSTOP "Diagnostic log cannot be created beside this EXE. Extract to a writable new folder. Nothing will be installed."
    Abort
FunctionEnd

Function .onGUIEnd
  FileWriteUTF16LE $DiagnosticHandle "event=diagnostic-exit$\r$\n"
  FileClose $DiagnosticHandle
FunctionEnd
