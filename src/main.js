const listen = window.__TAURI__.event.listen;
const invoke = window.__TAURI__.invoke;
const appWindow = window.__TAURI__.window.appWindow;

function loadText(textDescription) {
    // Get the text element
    let textElement = document.getElementById('main-msg');
    // Set the inner text of the text element to the provided description
    textElement.innerText = textDescription;
}

function loadImage(imageUrl) {
    // Get the image element
    let imgElement = document.getElementById('main-img');
    // Set the source of the image element to the provided URL
    imgElement.src = imageUrl;
}

function toggle_view(showChat) {
    var prompt = document.getElementById('prompt');
    var chat = document.getElementById('chat');

    if (showChat) {
        document.body.style.backgroundColor = "red";
        chat.style.display = "block"; // Show chat
        prompt.style.display = "none"; // Hide prompt
    } else {
        document.body.style.backgroundColor = "transparent";
        chat.style.display = "none"; // Show chat
        prompt.style.display = "block"; // Hide prompt
    }
}

window.addEventListener("DOMContentLoaded", () => {
    listen('chat_message', (event) => {
        // appWindow.show();
        loadText(event.payload);

        setTimeout(() => {
            appWindow.hide();
        }, 5000);
    });

    listen('send', (event) => {
        toggle_view(true);
    });

    listen('online_users', (event) => {
        console.log("received");
        var dropdown = document.getElementById('dropdown');
        var selected_option = document.getElementById('dropdown').value;

        dropdown.innerHTML = "";


        // Fill the dropdown with the string array in event.payload
        event.payload.forEach(function (item) {
            var option = document.createElement('option');
            option.text = item;
            option.value = item;

            if (item === selected_option) {
                dropdown.value = item
            }
            
            dropdown.add(option);
        });


    });
});


window.onload = function () {
    let thoughtDiv = document.querySelector('.thought');
    let style = window.getComputedStyle(thoughtDiv);
    let marginTop = parseInt(style.marginTop);
    let marginBottom = parseInt(style.marginBottom);
    let totalHeight = thoughtDiv.offsetHeight + marginTop + marginBottom;
    document.querySelector('.container').style.paddingTop = totalHeight + 'px';

    document.getElementById('submit').onclick = function () {
        var dropdown = document.getElementById('dropdown');
        var textbox = document.getElementById('textbox');
        invoke('send_message', {"receiver": dropdown.value, "text": textbox.value});
        toggle_view(false);

        appWindow.hide();
    };
};