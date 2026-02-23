// one global click handler
document.addEventListener("click", function (e) {
  const actionEle = e.target.closest("[data-action]");
  if (!actionEle) {
    // click on unhandlable elements
    closeAllDropdowns();
    return;
  }
  switch (actionEle.dataset.action) {
    case "toggle-dropdown":
      toggleDropdown(actionEle);
      e.stopPropagation();
      break;
    case "open-file":
      openFile(actionEle);
      break;
    case "download-file":
      downloadFile(actionEle);
      break;
  }
});

function downloadFile(btn) {
  const path = btn.dataset.path;

  // Fetch the file as blob
  fetch(path)
    .then((res) => res.blob())
    .then((blob) => {
      // Force type to application/octet-stream
      const url = URL.createObjectURL(
        new Blob([blob], { type: "application/octet-stream" }),
      );
      const a = document.createElement("a");
      a.href = url;
      a.download = path.split("/").pop(); // filename
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
    })
    .catch((err) => console.error("Download failed:", err));
}

function openFile(btn) {
  const fileUrl = btn.dataset.path;
  fetch(fileUrl).then((r) => r.text()).then((t) => {
    const win = window.open("", "_blank");
    win.document.writeln(
      "<pre>" +
        t.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;") +
        "</pre>",
    );
    win.document.close();
  });
}

function closeAllDropdowns() {
  document
    .querySelectorAll(".dropdown-content")
    .forEach((dc) => dc.style.display = "none");
}

function toggleDropdown(btn) {
  const content = btn.nextElementSibling;

  const isOpen = content.style.display == "block";
  content.style.display = isOpen ? "none" : "block";
}
