var Utils = (function() {
	var powers = '_KMGTPEZY';
	var monotime = function() { return Date.now(); };

	if (window.performance && window.performance.now)
		monotime = function() { return window.performance.now(); };

	return {
		debounce: function(callback, delay) {
			var timeout;
			var fn = function() {
				var context = this;
				var args = arguments;

				clearTimeout(timeout);
				timeout = setTimeout(function() {
					timeout = null;
					callback.apply(context, args);
				}, delay);
			};
			fn.clear = function() {
				clearTimeout(timeout);
				timeout = null;
			};

			return fn;
		},

		throttle: function(callback, delay) {
			var timeout;
			var last;
			var fn = function() {
				var context = this;
				var args = arguments;
				var now = monotime();

				if (last && now < last + delay) {
					clearTimeout(timeout);
					timeout = setTimeout(function() {
						timeout = null;
						last = now;
						callback.apply(context, args);
					}, delay);
				} else {
					last = now;
					callback.apply(context, args);
				}
			};
			fn.clear = function() {
				clearTimeout(timeout);
				timeout = null;
			};

			return fn;
		},

		format_number: function(number, digits) {
			if (digits === undefined) digits = 2;
			return '<span class="number">' + number.toLocaleString("en", {maximumFractionDigits: digits}).split(',').join('<span></span><wbr>') + '</span>';
		},

		bits_to_human: function(bits) {
			var bytes = Math.round(bits / 8);

			for (var i = powers.length - 1; i > 0; i--) {
				var div = Math.pow(2, 10*i);
				if (bytes >= div) {
					return Utils.format_number(bytes / div, 2) + powers[i] + 'iB';
				}
			}

			return Utils.format_number(bytes) + 'B';
		},

		human_to_bits: function(human) {
			if (!human) return null;
			var num = parseFloat(human);

			var match = (/\s*([KMGTPEZY])(i)?([Bb])?\s*$/i).exec(human);
			if (match) {
				var pow = (match[2] == 'i') ? 1024 : 1000;
				var mul = (match[3] == 'B') ? 8 : 1;

				num *= mul * Math.pow(pow, powers.indexOf(match[1].toUpperCase()));
			}

			return num;
		},

		number_to_human: function(num) {
			for (var i = powers.length - 1; i > 0; i--) {
				var div = Math.pow(10, 3*i);
				if (num >= div) {
					return Utils.format_number(num / div, 2) + powers[i];
				}
			}

			return num;
		},

		human_to_number: function(human) {
			if (!human) return null;
			var num = parseFloat(human);

			var match = (/\s*([KMGTPEZY])\s*$/i).exec(human);

			if (match) {
				num *= Math.pow(1000, powers.indexOf(match[1].toUpperCase()));
			}

			return num;
		},

		sformat: function() {
			var args = arguments;
			return args[0].replace(/\{(\d+)\}/g, function (m, n) { return args[parseInt(n) + 1]; });
		},

		range: function(a, b, step) {
			if (!step) step = 1;
			var arr = [];
			for (var i = a; i < b; i += step) {
				arr.push(i);
			}
			return arr;
		}
	};
})();

// Actions call back into Rust
var Action = (function() {
	return {
		open_url: function(url) {
			external.invoke(JSON.stringify({ type: 'OpenUrl', url: url }));
		},

		choose_folder: function() {
			external.invoke(JSON.stringify({ type: 'ChooseFolder' }));
		},

		compress: function() {
			external.invoke(JSON.stringify({ type: 'Compress' }));
		},

		decompress: function() {
			external.invoke(JSON.stringify({ type: 'Decompress' }));
		},

		pause: function() {
			external.invoke(JSON.stringify({ type: 'Pause' }));
		},

		continue: function() {
			external.invoke(JSON.stringify({ type: 'Continue' }));
		},

		cancel: function() {
			external.invoke(JSON.stringify({ type: 'Cancel' }));
		},

		quit: function() {
			external.invoke(JSON.stringify({ type: 'Quit' }));
		}
	};
})();

// Responses come from Rust
var Response = (function() {
	return {
		dispatch: function(msg) {
			switch(msg.type) {
				case "Folder":
					Gui.set_folder(msg.path);
					break;

				case "Progress":
					Gui.set_progress(msg.status, msg.pct);
					break;

				case "FolderInfo":
					break;
			}
		}
	};
})();

// Anything poking the GUI lives here
var Gui = (function() {
	return {
		boot: function() {
			$("a[href]").on("click", function(e) {
				e.preventDefault();
				Action.open_url($(this).attr("href"));
				return false;
			});
		},

		page: function(page) {
			$("nav button").removeClass("active");
			$("#Button_Page_" + page).addClass("active");
			$("section.page").hide();
			$("#" + page).show();
		},

		set_folder: function(folder) {
			var button = $("#Button_Folder");
			var bits = folder.split(/:\\|\\/);
			var end = bits.pop();

			button.empty();
			bits.forEach(function(bit) {
				button.append(document.createTextNode(bit));
				button.append($("<span>❱</span>"));
			});

			button.append(document.createTextNode(end));

			// why use a one-liner when you can faff about?
			// $("#Button_Folder").text(folder);
		},

		status_update: function(data) {
		},

		analysis_results: function(data) {
		},
	};
})();

$(document).ready(Gui.boot);
