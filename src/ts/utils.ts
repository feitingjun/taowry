/**
 * Utils - 系统工具类
 *
 * 提供文件对话框、系统通知、消息弹窗、打开文件/URL、OS 目录访问等实用功能。
 * 所有方法均为静态方法，通过 `import { Utils } from 'taowry'` 导入使用。
 */

import { native, json, parseOrNull } from './native-module.js'

/** 文件过滤器 */
export interface FilterItem {
  name: string
  extensions: string[]
}

/** 文件选择对话框选项 */
export interface PickFileOptions {
  /** 文件类型过滤器 */
  filters?: FilterItem[]
  /** 初始目录 */
  directory?: string
  /** 默认文件名 */
  fileName?: string
}

/** 保存文件对话框选项 */
export interface SaveFileOptions {
  /** 文件类型过滤器 */
  filters?: FilterItem[]
  /** 初始目录 */
  directory?: string
  /** 默认文件名 */
  fileName?: string
}

/** 消息对话框级别 */
export type MessageLevel = 'info' | 'warning' | 'error'

/** 消息对话框按钮配置 */
export type MessageButtons = 'ok' | 'okCancel' | 'yesNo' | 'yesNoCancel' | string[]

/** 消息对话框选项 */
export interface ShowMessageOptions {
  /** 标题 */
  title: string
  /** 内容 */
  body?: string
  /** 级别 */
  level?: MessageLevel
  /** 按钮配置 */
  buttons?: MessageButtons
}

export class Utils {
  // ===== 系统通知 =====

  /** 发送系统通知 */
  static notify(title: string, body?: string): void {
    native.utilsNotify(json({ title, body: body ?? '' }))
  }

  // ===== 打开操作 =====

  /** 用默认程序打开文件或文件夹 */
  static openFile(path: string): void {
    native.utilsOpenFile(path)
  }

  /** 用默认浏览器打开 URL */
  static openUrl(url: string): void {
    native.utilsOpenUrl(url)
  }

  // ===== 文件对话框 =====

  /** 选择单个文件 */
  static pickFile(options?: PickFileOptions): Promise<string | null> {
    return new Promise(resolve => {
      native.utilsPickFile(json(options ?? {}), (result: string) => {
        resolve(parseOrNull(result))
      })
    })
  }

  /** 选择多个文件 */
  static pickFiles(options?: PickFileOptions): Promise<string[]> {
    return new Promise(resolve => {
      native.utilsPickFiles(json(options ?? {}), (result: string) => {
        resolve(JSON.parse(result) as string[])
      })
    })
  }

  /** 选择文件夹 */
  static pickFolder(options?: { directory?: string }): Promise<string | null> {
    return new Promise(resolve => {
      native.utilsPickFolder(json(options ?? {}), (result: string) => {
        resolve(parseOrNull(result))
      })
    })
  }

  /** 保存文件对话框 */
  static saveFile(options?: SaveFileOptions): Promise<string | null> {
    return new Promise(resolve => {
      native.utilsSaveFile(json(options ?? {}), (result: string) => {
        resolve(parseOrNull(result))
      })
    })
  }

  // ===== 消息对话框 =====

  /**
   * 显示系统消息对话框
   *
   * @returns `ok`/`okCancel`/`yesNo` 返回 boolean；`yesNoCancel`/自定义按钮 返回 string
   */
  static showMessage(options: ShowMessageOptions): Promise<string | boolean> {
    return new Promise(resolve => {
      native.utilsShowMessage(json(options), (result: string) => {
        resolve(JSON.parse(result))
      })
    })
  }

  // ===== OS 标准目录 =====

  /** 获取桌面目录 */
  static getDesktopDir(): string | null {
    const r = native.utilsGetDir('desktop')
    return r === 'null' ? null : r
  }

  /** 获取文档目录 */
  static getDocumentsDir(): string | null {
    const r = native.utilsGetDir('documents')
    return r === 'null' ? null : r
  }

  /** 获取下载目录 */
  static getDownloadsDir(): string | null {
    const r = native.utilsGetDir('downloads')
    return r === 'null' ? null : r
  }

  /** 获取图片目录 */
  static getPicturesDir(): string | null {
    const r = native.utilsGetDir('pictures')
    return r === 'null' ? null : r
  }

  /** 获取音乐目录 */
  static getMusicDir(): string | null {
    const r = native.utilsGetDir('music')
    return r === 'null' ? null : r
  }

  /** 获取视频目录 */
  static getVideosDir(): string | null {
    const r = native.utilsGetDir('videos')
    return r === 'null' ? null : r
  }

  /** 获取用户主目录 */
  static getHomeDir(): string | null {
    const r = native.utilsGetDir('home')
    return r === 'null' ? null : r
  }

  /** 获取临时目录 */
  static getTempDir(): string | null {
    const r = native.utilsGetDir('temp')
    return r === 'null' ? null : r
  }

  // ===== 应用范围目录（需通过 ApplicationOptions.appName 设置，否则抛出异常）=====

  /** 获取应用数据目录 (macOS: ~/Library/Application Support/<appName>) */
  static getDataDir(): string {
    return native.utilsGetDir('appData')
  }

  /** 获取应用配置目录 (macOS: ~/Library/Preferences/<appName>) */
  static getConfigDir(): string {
    return native.utilsGetDir('appConfig')
  }

  /** 获取应用缓存目录 (macOS: ~/Library/Caches/<appName>) */
  static getCacheDir(): string {
    return native.utilsGetDir('appCache')
  }

  /** 获取应用日志目录 (macOS: ~/Library/Logs/<appName>) */
  static getLogDir(): string {
    return native.utilsGetDir('appLog')
  }
}
