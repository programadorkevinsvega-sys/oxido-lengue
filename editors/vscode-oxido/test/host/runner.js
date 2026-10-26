async function run() {
  try {
    await require('./extension.test.js').run();
  } catch (error) {
    console.error(error instanceof Error ? error.stack : String(error));
    throw error;
  }
}

module.exports = { run };
