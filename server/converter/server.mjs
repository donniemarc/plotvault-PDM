import http from 'node:http'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'
import occtImportJS from 'occt-import-js'

const PORT = Number(process.env.PORT || 8000)
const MAX_BODY = Number(process.env.MAX_BODY || 4 * 1024 * 1024 * 1024)

const here = dirname(fileURLToPath(import.meta.url))
const WASM = join(here, 'node_modules', 'occt-import-js', 'dist', 'occt-import-js.wasm')

let libPromise = null
function getLib() {
  if (!libPromise) {
    libPromise = occtImportJS({ locateFile: () => WASM })
  }
  return libPromise
}

function sendJson(res, status, data) {
  res.writeHead(status, { 'content-type': 'application/json; charset=utf-8' })
  res.end(JSON.stringify(data))
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://${req.headers.host || 'localhost'}`)
  if (req.method === 'GET' && url.pathname === '/health') {
    sendJson(res, 200, { ok: true })
    return
  }
  const m = url.pathname.match(/^\/convert\/(step|iges)$/)
  if (req.method !== 'POST' || !m) {
    sendJson(res, 404, { error: 'not found' })
    return
  }
  const format = m[1]
  let size = 0
  const chunks = []
  try {
    for await (const chunk of req) {
      size += chunk.length
      if (size > MAX_BODY) {
        sendJson(res, 413, { error: '文件过大' })
        return
      }
      chunks.push(chunk)
    }
  } catch (e) {
    sendJson(res, 400, { error: String((e && e.message) || e) })
    return
  }
  if (size === 0) {
    sendJson(res, 400, { error: '空请求体' })
    return
  }
  const buf = new Uint8Array(Buffer.concat(chunks))
  try {
    const lib = await getLib()
    const result = format === 'iges' ? lib.ReadIgesFile(buf, null) : lib.ReadStepFile(buf, null)
    if (!result.meshes || result.meshes.length === 0) {
      sendJson(res, 422, { error: '模型解析成功但未包含几何体' })
      return
    }
    res.writeHead(200, { 'content-type': 'application/json; charset=utf-8' })
    res.end(JSON.stringify({ meshes: result.meshes }))
  } catch (e) {
    sendJson(res, 500, { error: String((e && e.message) || e) })
  }
})

server.listen(PORT, () => {
  console.log(`plotvault-pdm converter listening on :${PORT}`)
})