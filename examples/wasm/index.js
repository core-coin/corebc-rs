const corebc = import("./pkg");

corebc
    .then(m => {
        m.deploy().catch(console.error);
    })
    .catch(console.error);
