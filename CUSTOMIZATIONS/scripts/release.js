#!/usr/bin/env node
/*---------------------------------------------------------------------------------------------
 *  custom-sidex —— release 打包脚本
 *
 *  作用：把版本号四处同步到目标版本，然后跑 `npm run build:custom` 出安装包，最后列出产物。
 *  产物在仓库根 target/release/bundle/（不是 src-tauri/target/，见 docs/pitfalls.md #4）。
 *
 *  用法（仓库根）：
 *     node CUSTOMIZATIONS/scripts/release.js 0.1.3-custom.5   # 显式指定版本
 *     node CUSTOMIZATIONS/scripts/release.js                  # 自动把 custom.N 递增 1
 *
 *  也可双击 CUSTOMIZATIONS/scripts/release.bat（等价，带 pause）。
 *
 *  版本号落点（四处，缺一不可）：
 *     package.json                       version
 *     src-tauri/tauri.conf.json          version
 *     src-tauri/tauri.conf.json          bundle.windows.wix.version（纯数字，custom.N → 0.1.3.N）
 *     src-tauri/Cargo.toml               [package] version
 *  注意：根 Cargo.toml 的 [workspace.package] version 是内部 crate 版本，不要动。
 *--------------------------------------------------------------------------------------------*/

import fs from 'fs';
import path from 'path';
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..', '..');
const PKG = path.join(ROOT, 'package.json');
const TAURI_CONF = path.join(ROOT, 'src-tauri', 'tauri.conf.json');
const CARGO_TOML = path.join(ROOT, 'src-tauri', 'Cargo.toml');
const BUNDLE_DIR = path.join(ROOT, 'target', 'release', 'bundle');

function fail(message) {
	console.error(`[X] ${message}`);
	process.exit(1);
}

function readJson(file) {
	return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function writeJson(file, value) {
	fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function bumpCargoToml(file, version) {
	const content = fs.readFileSync(file, 'utf8');
	// 只改 [package] 段里的 version（依赖里的 version = "..." 不能动）
	const section = content.match(/^\[package\][\s\S]*?(?=^\[)/m);
	if (!section) {
		fail(`在 ${path.relative(ROOT, file)} 里找不到 [package] 段`);
	}
	const versionRegex = /^version\s*=\s*"[^"]*"/m;
	if (!versionRegex.test(section[0])) {
		fail(`在 ${path.relative(ROOT, file)} 的 [package] 段里找不到 version 行`);
	}
	// 用「是否匹配」而非「replace 前后是否相等」判断：目标版本与当前一致时 replace 结果不变，属于正常幂等
	const updated = section[0].replace(versionRegex, `version = "${version}"`);
	fs.writeFileSync(file, content.replace(section[0], updated));
}

function walk(dir, out = []) {
	if (!fs.existsSync(dir)) {
		return out;
	}
	for (const name of fs.readdirSync(dir)) {
		const full = path.join(dir, name);
		const stat = fs.statSync(full);
		if (stat.isDirectory()) {
			walk(full, out);
		} else {
			out.push(full);
		}
	}
	return out;
}

function main() {
	// 1) 确定版本号
	let version = process.argv[2];
	if (version) {
		version = version.replace(/^v/, '');
	} else {
		const current = readJson(PKG).version;
		const matched = current.match(/^(.*-custom\.)(\d+)$/);
		if (!matched) {
			fail(`无法自动递增版本号 "${current}"，请显式传入：node CUSTOMIZATIONS/scripts/release.js 0.1.3-custom.N`);
		}
		version = `${matched[1]}${Number(matched[2]) + 1}`;
	}

	const vm = version.match(/^(\d+)\.(\d+)\.(\d+)-custom\.(\d+)$/);
	if (!vm) {
		fail(`版本号格式必须是 MAJOR.MINOR.PATCH-custom.N，例如 0.1.3-custom.5（收到：${version}）`);
	}
	const wix = `${vm[1]}.${vm[2]}.${vm[3]}.${vm[4]}`;

	// 2) 同步四处版本号
	const pkg = readJson(PKG);
	pkg.version = version;
	writeJson(PKG, pkg);

	const conf = readJson(TAURI_CONF);
	conf.version = version;
	if (!conf.bundle?.windows?.wix) {
		fail('tauri.conf.json 缺少 bundle.windows.wix 配置');
	}
	conf.bundle.windows.wix.version = wix;
	writeJson(TAURI_CONF, conf);

	bumpCargoToml(CARGO_TOML, version);

	// 3) 打印新版本号供核对
	console.log('=== 版本号已同步（四处）===');
	console.log(`  package.json             : ${version}`);
	console.log(`  tauri.conf.json version  : ${version}`);
	console.log(`  tauri wix.version        : ${wix}`);
	console.log(`  src-tauri/Cargo.toml     : ${version}`);
	console.log('');

	// 4) 打包
	console.log('=== 开始打包（npm run build:custom，已内置 AWS_LC_SYS_PREBUILT_NASM=1）===');
	// Windows 下 npm 是 npm.cmd，spawnSync 直接 spawn 会 EINVAL，必须 shell:true 走 cmd.exe
	const result = spawnSync('npm', ['run', 'build:custom'], {
		stdio: 'inherit',
		cwd: ROOT,
		env: process.env,
		shell: true
	});
	if (result.status !== 0) {
		fail(`打包失败（退出码 ${result.status}）`);
	}

	// 5) 列出产物
	console.log('');
	console.log('=== 产物 ===');
	const artifacts = walk(BUNDLE_DIR).filter(f => /\.(msi|exe|dmg|AppImage|deb)$/i.test(f));
	if (artifacts.length === 0) {
		console.log('  [X] 未在 target/release/bundle/ 找到安装包（注意看上面是否出现 "Finished N bundles at:"）');
		process.exit(1);
	}
	for (const f of artifacts) {
		const size = (fs.statSync(f).size / 1024 / 1024).toFixed(1);
		console.log(`  ${f}  (${size} MB)`);
	}
	console.log('');
	console.log('提示：判断成功看上面是否有 "Finished N bundles at:"，编译成功 != 打包成功（见 pitfalls.md #4）。');
}

main();
