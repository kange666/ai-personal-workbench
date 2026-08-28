const { test, expect } = require('@playwright/test')

const authToken = process.env.E2E_AUTH_TOKEN || process.env.HLZT_TOKEN || ''
const menuCase = {
  menuName: process.env.E2E_MENU_NAME || '当前业务',
  route: process.env.E2E_MENU_ROUTE || '/',
  component: process.env.E2E_MENU_COMPONENT || '',
  layoutFlag: process.env.E2E_MENU_LAYOUT_FLAG || 'businessLayout'
}
let resolvedMenuRoute = menuCase.route

function joinRoutePath(parentPath, routePath) {
  const currentPath = String(routePath || '').trim()
  if (!currentPath) return parentPath || '/'
  if (/^https?:\/\//i.test(currentPath) || currentPath.startsWith('/')) return currentPath
  return `${String(parentPath || '').replace(/\/+$/, '')}/${currentPath}`.replace(/\/{2,}/g, '/')
}

function flattenMenuRoutes(routes, parentPath = '') {
  return (Array.isArray(routes) ? routes : []).flatMap((route) => {
    const fullPath = joinRoutePath(parentPath, route.path)
    return [{
      fullPath,
      component: String(route.component || ''),
      title: String(route.meta?.title || ''),
      layoutFlag: String(route.layoutFlag || '')
    }].concat(flattenMenuRoutes(route.children, fullPath))
  })
}

async function resolveCurrentRoute() {
  if (!authToken) throw new Error('真实接口测试需要设置 Windows 用户环境变量 HLZT_TOKEN。')
  const baseUrl = process.env.E2E_BASE_URL || 'http://127.0.0.1:5182'
  const response = await fetch(new URL('/dev-api/menu/getRouters', baseUrl), {
    headers: { 'hlzt-token': authToken }
  }).catch((error) => {
    throw new Error(`动态菜单前置检查失败：${error.message}`)
  })
  expect(response.status, '动态菜单接口返回了服务端错误').toBeLessThan(500)
  const body = await response.json().catch(() => null)
  expect(body?.code ?? 200, '动态菜单接口业务状态异常').toBe(200)
  const routes = flattenMenuRoutes(body?.data)
  const byComponent = menuCase.component
    ? routes.filter((item) => item.component === menuCase.component)
    : []
  const candidates = byComponent.length
    ? byComponent
    : routes.filter((item) => item.title === menuCase.menuName)
  const resolved = candidates.find((item) => item.fullPath === menuCase.route)
    || candidates.find((item) => item.layoutFlag === menuCase.layoutFlag)
    || candidates[0]
  if (!resolved) {
    throw new Error(`当前账号动态菜单中未找到“${menuCase.menuName}”，请检查账号权限或菜单配置。`)
  }
  return resolved.fullPath
}

function isBusinessResponse(response) {
  const pathname = new URL(response.url()).pathname
  // 业务项目同时使用 /dev-api、/dev-exam 等 Vite 代理，公共用例需要统一识别。
  return pathname.startsWith('/dev-') && pathname !== '/dev-api/menu/getRouters'
}

async function openCurrentPage(page) {
  await page.context().addCookies([{
    name: 'Admin-Token',
    value: authToken,
    domain: '127.0.0.1',
    path: '/'
  }])
  await page.addInitScript((routerType) => {
    window.localStorage.setItem('routerType', routerType)
  }, menuCase.layoutFlag)
  await page.goto('/index')
  await page.waitForLoadState('networkidle', { timeout: 15000 }).catch(() => {})
  await page.goto(resolvedMenuRoute)
  await expect(page.getByText('404错误')).toHaveCount(0)
  await expect(page.locator('.app-container').first()).toBeVisible({ timeout: 30000 })
}

async function optionalActionButton(page, text) {
  const exact = page.locator('.query-form:visible .el-button, .app-container .el-form:visible .el-button')
    .filter({ hasText: text })
    .first()
  return await exact.count() ? exact : null
}

test.describe(`公共通用真实接口测试：${menuCase.menuName}`, () => {
  test.beforeAll(async () => {
    resolvedMenuRoute = await resolveCurrentRoute()
  })

  test('真实登录态可以进入当前业务页面', async ({ page }) => {
    await openCurrentPage(page)
    await expect(page.locator('.app-container').first()).toBeVisible()
  })

  test('页面首屏真实接口没有服务端错误', async ({ page }) => {
    const responses = []
    page.on('response', (response) => {
      if (isBusinessResponse(response)) responses.push(response)
    })
    await openCurrentPage(page)
    await page.waitForTimeout(1200)
    expect(responses.length, '页面首屏没有观察到真实业务接口请求').toBeGreaterThan(0)
    expect(responses.filter((response) => response.status() >= 500).map((response) => response.url()), '真实接口存在服务端错误').toEqual([])
  })

  test('查询操作会调用真实接口（页面存在查询按钮时）', async ({ page }) => {
    await openCurrentPage(page)
    const button = await optionalActionButton(page, '查询') || await optionalActionButton(page, '搜索')
    test.skip(!button, '当前页面没有查询或搜索按钮。')
    const responsePromise = page.waitForResponse(isBusinessResponse, { timeout: 30000 })
    await button.click()
    const response = await responsePromise
    expect(response.status()).toBeLessThan(500)
  })

  test('重置查询后页面可以继续使用（页面存在重置按钮时）', async ({ page }) => {
    await openCurrentPage(page)
    const button = await optionalActionButton(page, '重置')
    test.skip(!button, '当前页面没有重置按钮。')
    await button.click()
    await expect(page.locator('.app-container').first()).toBeVisible()
  })

  test('页面没有 JavaScript 运行时错误', async ({ page }) => {
    const errors = []
    page.on('pageerror', (error) => errors.push(error.message))
    await openCurrentPage(page)
    await page.waitForTimeout(800)
    expect(errors, `页面存在运行时错误：${JSON.stringify(errors, null, 2)}`).toEqual([])
  })

  test('桌面视口下页面主体没有横向溢出', async ({ page }) => {
    await openCurrentPage(page)
    const layout = await page.evaluate(() => ({
      clientWidth: document.documentElement.clientWidth,
      scrollWidth: document.documentElement.scrollWidth
    }))
    expect(layout.scrollWidth).toBeLessThanOrEqual(layout.clientWidth + 2)
  })
})
