import { options, prepare } from './build-common.mjs'

try {
  await prepare(options(process.argv.slice(2)))
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
}
