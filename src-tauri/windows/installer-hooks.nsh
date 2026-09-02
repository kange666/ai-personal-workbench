; V1.9.2 使用旧产品身份修复了安装目录切换问题。
; 从 V1.9.3 起恢复“星枢工作台”品牌，但安装时优先继承旧产品目录，
; 避免再次回到默认的 LocalAppData 目录。
!define LEGACY_PRODUCT_NAME "AI 个人工作台"
!define LEGACY_UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${LEGACY_PRODUCT_NAME}"

!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $R8 HKCU "Software\${MANUFACTURER}\${LEGACY_PRODUCT_NAME}" ""
  ${If} $R8 != ""
    DetailPrint "正在沿用原 AI 个人工作台安装目录..."

    ; 兼容尚未安装 V1.9.2、仍同时存在两套安装的用户。
    ReadRegStr $R6 HKCU "${UNINSTKEY}" "UninstallString"
    ReadRegStr $R7 HKCU "${MANUPRODUCTKEY}" ""
    ${If} $R6 != ""
    ${AndIf} $R7 != ""
    ${AndIf} $R7 != $R8
      DetailPrint "正在移除旧的重复星枢安装..."
      ClearErrors
      ExecWait '$R6 /UPDATE /P _?=$R7' $R5
      ${If} ${Errors}
        Abort "无法启动旧星枢安装的卸载程序，请关闭应用后重试。"
      ${ElseIf} $R5 != 0
        Abort "旧星枢安装清理失败，返回代码 $R5。"
      ${EndIf}
    ${EndIf}

    ReadRegStr $R9 HKCU "${LEGACY_UNINSTKEY}" "UninstallString"
    ${If} $R9 != ""
      DetailPrint "正在迁移 AI 个人工作台安装记录..."
      ClearErrors
      ExecWait '$R9 /UPDATE /P _?=$R8' $R5
      ${If} ${Errors}
        Abort "无法启动 AI 个人工作台卸载程序，请关闭应用后重试。"
      ${ElseIf} $R5 != 0
        Abort "AI 个人工作台安装记录迁移失败，返回代码 $R5。"
      ${EndIf}
    ${EndIf}

    ; 旧卸载程序可能删除安装目录，因此需要重新创建并重设输出目录。
    StrCpy $INSTDIR $R8
    CreateDirectory "$INSTDIR"
    SetOutPath "$INSTDIR"

    ; 默认安装器不要额外创建快捷方式，现有快捷方式会在安装后原地改名。
    StrCpy $NoShortcutMode 1
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  IfFileExists "$SMPROGRAMS\${LEGACY_PRODUCT_NAME}.lnk" 0 start_menu_done
    Delete "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    Rename "$SMPROGRAMS\${LEGACY_PRODUCT_NAME}.lnk" "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    DetailPrint "开始菜单快捷方式已改名为 ${PRODUCTNAME}。"
  start_menu_done:

  IfFileExists "$DESKTOP\${LEGACY_PRODUCT_NAME}.lnk" 0 desktop_done
    Delete "$DESKTOP\${PRODUCTNAME}.lnk"
    Rename "$DESKTOP\${LEGACY_PRODUCT_NAME}.lnk" "$DESKTOP\${PRODUCTNAME}.lnk"
    DetailPrint "桌面快捷方式已改名为 ${PRODUCTNAME}。"
  desktop_done:

  ; 新品牌注册完成后移除旧产品的残留注册项，避免“已安装应用”显示两条。
  DeleteRegKey HKCU "${LEGACY_UNINSTKEY}"
  DeleteRegKey HKCU "Software\${MANUFACTURER}\${LEGACY_PRODUCT_NAME}"
!macroend
