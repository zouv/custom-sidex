/*---------------------------------------------------------------------------------------------
 *  custom-sidex —— 从本地 .vsix 安装扩展
 *
 *  为什么需要这个模块：
 *  SideX 前端跑在 Web 模式（`isWeb === true`），`ExtensionManagementServerService` 只注册了
 *  web 扩展服务器，`localExtensionManagementServer` 恒为 null。于是上游 VSCode 的 VSIX 安装链路
 *  在本项目里整体断掉：
 *    - 「Install from VSIX...」菜单的 when 条件要求 hasLocalServer || hasRemoteServer（全为 false），
 *      命令与菜单项都不会出现；
 *    - `ExtensionManagementService.getManifest(vsix)` 对 file:// 直接 reject('No Servers')，
 *      `installVSIX()` → `getServersToInstall()` 同样 reject('No Servers to Install')。
 *  Rust 侧的安装能力其实早已具备（`src-tauri/src/commands/extensions.rs` 的 `install_extension` →
 *  `crates/sidex-extensions/src/vsix.rs`，解压到 %APPDATA%\SideX\extensions\<publisher>.<name>），
 *  只是前端从未接过线。
 *
 *  本模块把两端接起来（不改上游扩展管理逻辑）：
 *    1. 命令面板 / 扩展视图标题菜单「...」/ 资源管理器右键 .vsix / 把 .vsix 拖进窗口
 *    2. 原生文件对话框选包（IFileDialogService 已桥接 Tauri dialog 插件）
 *    3. Tauri `install_extension` 解压到扩展目录
 *    4. web 扩展服务的 `installFromLocation(已装目录)`：
 *         - 写入扩展视图的已安装记录（IndexedDB，location 指向磁盘目录）
 *         - 触发 onDidInstallExtensions → `tauriExtensionHost.contribution.ts` 既有的热加载逻辑
 *           （loadExtension + activateExtension + 注册 contributes.commands）
 *  卸载沿用既有链路：扩展视图卸载 → onDidUninstallExtension → Tauri `uninstall_extension`。
 *--------------------------------------------------------------------------------------------*/

import { getErrorMessage } from 'vs/base/common/errors.js';
import { Disposable, toDisposable } from 'vs/base/common/lifecycle.js';
import * as path from 'vs/base/common/path.js';
import Severity from 'vs/base/common/severity.js';
import { URI } from 'vs/base/common/uri.js';
import { localize, localize2 } from 'vs/nls.js';
import { MenuId, MenuRegistry } from 'vs/platform/actions/common/actions.js';
import { CommandsRegistry, ICommandService } from 'vs/platform/commands/common/commands.js';
import { ContextKeyExpr } from 'vs/platform/contextkey/common/contextkey.js';
import { ServicesAccessor } from 'vs/platform/instantiation/common/instantiation.js';
import { IFileDialogService } from 'vs/platform/dialogs/common/dialogs.js';
import { INotificationService } from 'vs/platform/notification/common/notification.js';
import { IProgressService, ProgressLocation } from 'vs/platform/progress/common/progress.js';
import { IWorkbenchContribution, registerWorkbenchContribution2, WorkbenchPhase } from 'vs/workbench/common/contributions.js';
import { ResourceContextKey } from 'vs/workbench/common/contextkeys.js';
import { VIEWLET_ID } from 'vs/workbench/contrib/extensions/common/extensions.js';
import { IExtensionManagementServer, IExtensionManagementServerService } from 'vs/workbench/services/extensionManagement/common/extensionManagement.js';
import { IUserDataProfileService } from 'vs/workbench/services/userDataProfile/common/userDataProfile.js';

export const INSTALL_VSIX_COMMAND_ID = 'sidex.extensions.installFromVSIX';

const isSidexTauri = (globalThis as any).__SIDEX_TAURI__ === true;

const ExtensionsCategory = {
	value: localize('extensions', 'Extensions'),
	original: 'Extensions'
};

/** Tauri `install_extension` 命令的返回值（见 src-tauri/src/commands/extensions.rs） */
interface InstalledVsix {
	id: string;
	name: string;
	version: string;
	/** 安装落地的目录（%APPDATA%\SideX\extensions\<publisher>.<name>） */
	path: string;
}

/**
 * 命令入口一次性解析出的服务集合。
 * ⚠️ `ServicesAccessor` 只在**同步调用栈内**有效，`await` 之后再 `accessor.get(...)` 会抛
 * "Illegal state: service accessor is only valid during the invocation of its target method"，
 * 所以所有服务必须在进入命令时同步取出，再传下去用。
 */
interface InstallContext {
	notificationService: INotificationService;
	progressService: IProgressService;
	fileDialogService: IFileDialogService;
	extensionManagementServer: IExtensionManagementServer;
	profileLocation: URI;
}

function createInstallContext(accessor: ServicesAccessor): InstallContext {
	const extensionManagementServer = accessor.get(IExtensionManagementServerService).webExtensionManagementServer;
	if (!extensionManagementServer) {
		throw new Error(localize('sidex.installVSIX.noServer', 'No extension management server is available.'));
	}
	return {
		notificationService: accessor.get(INotificationService),
		progressService: accessor.get(IProgressService),
		fileDialogService: accessor.get(IFileDialogService),
		extensionManagementServer,
		profileLocation: accessor.get(IUserDataProfileService).currentProfile.extensionsResource
	};
}

/**
 * 调用 Tauri 把 .vsix 解压到扩展目录。只做磁盘安装，不写扩展视图记录。
 */
async function installVsixToDisk(vsixPath: string): Promise<InstalledVsix> {
	const { invoke } = await import('@tauri-apps/api/core');
	return invoke<InstalledVsix>('install_extension', { vsixPath });
}

/**
 * 把已落地的扩展目录登记进扩展管理链路：
 * 写入已安装记录 → 触发 onDidInstallExtensions → 扩展宿主热加载并激活。
 */
async function registerInstalledExtension(context: InstallContext, installDir: string): Promise<void> {
	await context.extensionManagementServer.extensionManagementService.installFromLocation(
		URI.file(installDir),
		context.profileLocation
	);
}

async function installVsixPaths(context: InstallContext, vsixPaths: string[]): Promise<void> {
	await context.progressService.withProgress(
		{
			location: ProgressLocation.Notification,
			title: localize('sidex.installVSIX.progress', 'Installing extension from VSIX...')
		},
		async progress => {
			for (const vsixPath of vsixPaths) {
				const fileName = path.basename(vsixPath);
				progress.report({ message: fileName });
				try {
					const installed = await installVsixToDisk(vsixPath);
					// 升级/重装时磁盘目录已就位，这里只负责让扩展视图与扩展宿主看到它
					await registerInstalledExtension(context, installed.path);
					context.notificationService.notify({
						severity: Severity.Info,
						message: localize(
							'sidex.installVSIX.done',
							"Installed extension '{0}' ({1}).",
							installed.name || installed.id,
							installed.version
						)
					});
				} catch (error) {
					context.notificationService.error(
						localize(
							'sidex.installVSIX.failed',
							"Failed to install '{0}': {1}",
							fileName,
							getErrorMessage(error)
						)
					);
				}
			}
		}
	);
}

if (isSidexTauri) {
	CommandsRegistry.registerCommand({
		id: INSTALL_VSIX_COMMAND_ID,
		metadata: {
			description: localize('sidex.installVSIX.description', 'Install an extension from a local .vsix package')
		},
		handler: async (accessor: ServicesAccessor, ...resources: (URI | string)[]) => {
			// 注意：以下 accessor.get 全部发生在第一个 await 之前（accessor 只在同步栈内有效）
			const notificationService = accessor.get(INotificationService);
			let context: InstallContext;
			try {
				context = createInstallContext(accessor);
			} catch (error) {
				notificationService.error(localize('sidex.installVSIX.noServerError', 'Cannot install VSIX: {0}', getErrorMessage(error)));
				return;
			}

			const vsixPaths = resources
				.map(resource => (URI.isUri(resource) ? resource.fsPath : typeof resource === 'string' ? resource : undefined))
				.filter((value): value is string => !!value && value.toLowerCase().endsWith('.vsix'));

			if (vsixPaths.length > 0) {
				await installVsixPaths(context, vsixPaths);
				return;
			}

			const picked = await context.fileDialogService.showOpenDialog({
				title: localize('sidex.installVSIX.pickTitle', 'Install from VSIX'),
				filters: [{ name: 'VSIX Extensions', extensions: ['vsix'] }],
				canSelectFiles: true,
				canSelectMany: true
			});
			if (!picked?.length) {
				return;
			}
			await installVsixPaths(context, picked.map(uri => uri.fsPath));
		}
	});

	// 命令面板
	MenuRegistry.appendMenuItem(MenuId.CommandPalette, {
		command: {
			id: INSTALL_VSIX_COMMAND_ID,
			title: localize2('sidex.installVSIX.commandTitle', 'Install Extension from VSIX...'),
			category: ExtensionsCategory
		}
	});

	// 扩展视图标题栏的「...」菜单（对齐上游 hasLocalServer 时才出现的那一项）
	MenuRegistry.appendMenuItem(MenuId.ViewContainerTitle, {
		command: {
			id: INSTALL_VSIX_COMMAND_ID,
			title: localize('sidex.installVSIX.shortTitle', 'Install from VSIX...')
		},
		when: ContextKeyExpr.equals('viewContainer', VIEWLET_ID),
		group: '3_install',
		order: 1
	});

	// 资源管理器里右键 .vsix
	MenuRegistry.appendMenuItem(MenuId.ExplorerContext, {
		command: {
			id: INSTALL_VSIX_COMMAND_ID,
			title: localize('sidex.installVSIX.contextTitle', 'Install Extension VSIX')
		},
		when: ResourceContextKey.Extension.isEqualTo('.vsix'),
		group: 'extensions'
	});
}

/**
 * 拖拽安装：Tauri 的 `dragDropEnabled` 默认接管 OS 拖放，webview 收不到 HTML5 drop 事件
 * （`extensionsViewlet.ts` 里那个 DropObserver 在本项目里不会触发），只能监听 Tauri 事件。
 * 只处理 .vsix，其它文件保持原有行为（当前是不响应）。
 */
class InstallVsixOnDropContribution extends Disposable implements IWorkbenchContribution {

	static readonly ID = 'sidex.installVsixOnDrop';

	constructor(@ICommandService private readonly commandService: ICommandService) {
		super();
		this._registerDropListener();
	}

	private _registerDropListener(): void {
		void (async () => {
			try {
				const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
				const unlisten = await getCurrentWebviewWindow().onDragDropEvent(event => {
					if (event.payload.type !== 'drop') {
						return;
					}
					const vsixPaths = (event.payload.paths ?? []).filter(p => p.toLowerCase().endsWith('.vsix'));
					if (vsixPaths.length === 0) {
						return;
					}
					this.commandService
						.executeCommand(INSTALL_VSIX_COMMAND_ID, ...vsixPaths.map(p => URI.file(p)))
						.catch(() => { /* 失败已在命令内弹通知 */ });
				});
				if (this._store.isDisposed) {
					unlisten();
				} else {
					this._register(toDisposable(unlisten));
				}
			} catch {
				// 非 Tauri 环境或事件不可用时静默降级：菜单/命令面板仍可用
			}
		})();
	}
}

if (isSidexTauri) {
	registerWorkbenchContribution2(
		InstallVsixOnDropContribution.ID,
		InstallVsixOnDropContribution,
		WorkbenchPhase.AfterRestored
	);
}
