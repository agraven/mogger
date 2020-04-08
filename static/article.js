"use strict";

function auto_resize(element) {
	element.style.height = "auto";
	element.style.height = (element.scrollHeight + 10) + "px";
}

// Iterates over the key-value pairs in a FormData object and converts them to
// an object, using the data-type attribute on form elements to convert to
// appropriate data types.
function formToObject(data, form) {
	let object = {};
	data.forEach(function(value, key) {
		if (form.querySelector('[name="'+key+'"]').getAttribute("data-type") == "int") {
			value = parseInt(value);
		}
		if (form.querySelector('[name="'+key+'"]').getAttribute("data-type") == "bool") {
			if (value == "false") {
				value = false;
			}
			if (value == "true") {
				value = true;
			}
		}
		object[key] = value;
	})
	return object;
}

// Sends reply form data to the JSON API point
function send(form) {
	let data = new FormData(form);
	let request = new XMLHttpRequest();
	let object = formToObject(data, form);

	// Error handling
	const errorHandler = function() {
		form.querySelector(".error").innerHTML = "An error happened: " + this.statusText;
	}

	request.addEventListener('error', errorHandler, false);
	// Success handling
	request.addEventListener('loadend', function() {
		if (request.status !== 200) {
			form.querySelector(".error").innerHTML = "An error happened: "+request.statusText+": "+request.responseText;
			return;
		}
		let comment = JSON.parse(this.response);
		renderRequest.open("GET", "/api/comments/render/" + comment.id);
		renderRequest.send();
	}, false);

	let renderRequest = new XMLHttpRequest();
	renderRequest.addEventListener('error', errorHandler, false);
	renderRequest.addEventListener('loadend', function() {
		if (this.status !== 200) {
			form.querySelector(".error").innerHTML = "An error happened: "+this.statusText+": "+this.responseText;
			return;
		}
		form.insertAdjacentHTML('afterend', this.response);
		form.style.display = 'none';
		addCommentListeners(form.nextElementSibling);
	}, false);

	// Prepare the request
	request.open("POST", "/api/comments/submit");
	// Convert form data to object
	// Send data
	request.send(JSON.stringify(object));
}

function reply(comment) {
	let form = comment.querySelector("form.comment.reply");
	form.style.display = "block";
}

function edit(comment) {
	let request = new XMLHttpRequest();
	request.addEventListener('loadend', function() {
		let body = comment.querySelector("div.body");
		let form = comment.querySelector("form.comment.edit");
		let response = JSON.parse(this.response);
		body.style.display = "none";
		form.style.display = "block";
		form.querySelector("textarea").value = response.content;
	}, false);
	request.open("GET", "/api/comments/single/" + comment.getAttribute("data-id"));
	request.send();
}

function sendEdit(contentElement, form, id) {
	let data = new FormData(form);
	let request = new XMLHttpRequest();
	let req2 = new XMLHttpRequest();
	let messageElement = form.querySelector(".error");
	let object = formToObject(data, form);
	request.addEventListener('error', function() {
		messageElement.innerHTML = "A connection error happened: " + request.statusText;
	}, false);

	request.addEventListener('loadend', function() {
		if (request.status != 200) {
			messageElement.innerHTML = "An error happened: " + this.responseText;
			return;
		}
		req2.send();
	}, false);

	// Update content element on succssful submission
	req2.addEventListener('loadend', function() {
		if (req2.status != 200) {
			messageElement.innerHTML = "An error happened: " + this.responseText;
			return;
		}
		contentElement.innerHTML = this.response;
		form.style.display = 'none';
		contentElement.style.display = 'block';
	})
	req2.open("GET", "/api/comments/render-content/" + id)

	request.open("POST", "/api/comments/edit/" + id);
	request.send(JSON.stringify(object))
}

function remove(comment) {
	let request = new XMLHttpRequest();
	request.open("GET", "/api/comments/delete/" + comment.getAttribute("data-id"));
	request.send();
}

function purge(comment) {
	if (confirm("This will permanently delete the selected comment and cannot be undone. Continue?")) {
		let request = new XMLHttpRequest();
		request.open("GET", "/api/comments/purge/" + comment.getAttribute("data-id"));
		request.send();
	}
}

function addCommentListeners(comment) {
	// Add listeners to comment buttons
	let replyButton = comment.querySelector('button[data-action="reply"]');
	if (replyButton !== null) {
		replyButton.addEventListener('click', function() {
			reply(comment);
		}, false);
	}
	let editButton = comment.querySelector('button[data-action="edit"]');
	if (editButton !== null) {
		editButton.addEventListener('click', function() {
			edit(comment);
		}, false);
	}
	let removeButton = comment.querySelector('button[data-action="remove"]');
	if (removeButton !== null) {
		removeButton.addEventListener('click', function() {
			remove(comment);
		}, false);
	}
	let purgeButton = comment.querySelector('button[data-action="purge"]');
	if (purgeButton !== null) {
		purgeButton.addEventListener('click', function() {
			purge(comment);
		}, false);
	}

	// Add listeners to edit form buttons
	let editForm = comment.querySelector("form.comment.edit");
	editForm.querySelector('button[data-action="cancel"]').addEventListener('click', function() {
		comment.querySelector("div.body").style.display = "block";
		comment.querySelector("form.comment.edit").style.display = "none";
	}, false);
	editForm.querySelector('button[data-action="save"]').addEventListener('click', function() {
		sendEdit(comment.querySelector("div.body"), editForm, comment.getAttribute("data-id"));
	}, false);

	// Add listeners to reply form buttons
	let replyForm = comment.querySelector("form.comment.reply");
	replyForm.querySelector('button[data-action="submit"]').addEventListener('click', function() {
		send(replyForm);
	}, false);
	replyForm.querySelector('button[data-action="cancel"]').addEventListener('click', function() {
		replyForm.style.display = 'none';
	});
}

// Add listeners to buttons
function init() {
	let topComment = document.getElementById('new-comment');
	topComment.querySelector('button[data-action="submit"]').addEventListener('click', function() {
		send(topComment);
	}, false);
	for (let textarea of document.querySelectorAll("textarea")) {
		textarea.addEventListener('input', function() { auto_resize(this) }, false);
		auto_resize(textarea);
	}
	for (let comment of document.querySelectorAll("div.comment")) {
		addCommentListeners(comment);
	}
}

// Only run when DOM has loaded
if (document.readyState === 'loading') {
	document.addEventListener('DOMContentLoaded', init);
} else {
	init();
}
