; MapleLink NSIS installer hooks.
;
; Everything MapleLink writes lives under $INSTDIR or the per-user app data
; directory, with one exception: Locale Remulator cannot load its hook DLL from
; a path outside the ANSI code page (it round-trips that path through
; GetModuleFileNameA into `rundll32.exe "%hs",#1`), so when the Windows profile
; name is not ASCII the LR files are extracted to an ASCII root instead — see
; ASCII_ROOTS / ASCII_FALLBACK_DIR in src-tauri/src/services/lr_service.rs.
;
; That directory is outside $INSTDIR, so the uninstaller has to remove it
; explicitly. Only the "lr" folder we wrote is deleted, then its parent if that
; leaves it empty — never a recursive delete of a root we did not create.
; Failures are ignored: an unelevated per-user uninstall may not be allowed to
; touch %ProgramData%, and a leftover folder is not worth a failed uninstall.

!macro NSIS_HOOK_POSTUNINSTALL
  SetDetailsPrint textonly
  ClearErrors

  ; %ProgramData%\MapleLink\lr  (also covers %ALLUSERSPROFILE%, the same folder)
  ReadEnvStr $R0 "ProgramData"
  StrCmp $R0 "" maplelink_lr_systemdrive
  IfFileExists "$R0\MapleLink\lr\*.*" 0 maplelink_lr_systemdrive
  RMDir /r "$R0\MapleLink\lr"
  RMDir "$R0\MapleLink"

maplelink_lr_systemdrive:
  ; %SystemDrive%\MapleLink\lr  (used when %ProgramData% was not writable)
  ReadEnvStr $R0 "SystemDrive"
  StrCmp $R0 "" maplelink_lr_done
  IfFileExists "$R0\MapleLink\lr\*.*" 0 maplelink_lr_done
  RMDir /r "$R0\MapleLink\lr"
  RMDir "$R0\MapleLink"

maplelink_lr_done:
  ClearErrors
  SetDetailsPrint both
!macroend
