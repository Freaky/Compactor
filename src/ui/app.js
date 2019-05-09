var Util = (function() {
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
			return number.toLocaleString("en", {minimumFractionDigits: digits, maximumFractionDigits: digits});
		},

		bytes_to_human: function(bytes) {
			for (var i = powers.length - 1; i > 0; i--) {
				var div = Math.pow(2, 10*i);
				if (bytes >= div) {
					return Util.format_number(bytes / div, 2) + " " + powers[i] + 'iB';
				}
			}

			return Util.format_number(bytes, 0) + ' B';
		},

		human_to_bytes: function(human) {
			if (!human) return null;
			var num = parseFloat(human);

			var match = (/\s*([KMGTPEZY])(i)?([Bb])?\s*$/i).exec(human);
			if (match) {
				var pow = (match[2] == 'i') ? 1024 : 1000;
				// var mul = (match[3] == 'B') ? 8 : 1;

				num *= Math.pow(pow, powers.indexOf(match[1].toUpperCase()));
			}

			return num;
		},

		number_to_human: function(num) {
			for (var i = powers.length - 1; i > 0; i--) {
				var div = Math.pow(10, 3*i);
				if (num >= div) {
					return Util.format_number(num / div, 2) + powers[i];
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

		resume: function() {
			external.invoke(JSON.stringify({ type: 'Resume' }));
		},

		analyse: function() {
			external.invoke(JSON.stringify({ type: 'Analyse' }));
		},

		stop: function() {
			external.invoke(JSON.stringify({ type: 'Stop' }));
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

				case "Status":
					Gui.set_status(msg.status, msg.pct);
					break;

				case "Paused":
				case "Resumed":
				case "Stopped":
				case "Scanned":
					Gui[msg.type.toLowerCase()]();
					break;

				case "FolderSummary":
					Gui.set_folder_summary(msg.info);
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
			// I swear this worked earlier :(
			// var bits = folder.split(/:\\|\\/).map(document.createTextNode);
			var bits = folder.split(/:\\|\\/).map(function(x) { return document.createTextNode(x); });
			var end = bits.pop();

			var button = $("#Button_Folder");
			button.empty();
			bits.forEach(function(bit) {
				button.append(bit);
				button.append($("<span>❱</span>"));
			});
			button.append(end);

			Gui.scanning();
		},

		set_status: function(status, pct) {
			$("#Activity_Text").text(status);
			if (pct != null) {
				$("#Activity_Progress").val(pct);
			} else {
				$("#Activity_Progress").removeAttr("value");
			}
		},

		scanning: function() {
			Gui.reset_folder_summary();
			$("#Activity").show();
			$("#Analysis").show();
			$("#Button_Pause").show();
			$("#Button_Resume").hide();
			$("#Button_Stop").show();
			$("#Button_Analyse").hide();
			$("#Button_Compress").hide();
			$("#Button_Decompress").hide();
			$("#Command").show();
		},

		paused: function() {
			$("#Button_Pause").hide();
			$("#Button_Resume").show();
		},

		resumed: function() {
			$("#Button_Pause").show();
			$("#Button_Resume").hide();
		},

		stopped: function() {
			Gui.scanned();
		},

		scanned: function() {
			$("#Button_Pause").hide();
			$("#Button_Resume").hide();
			$("#Button_Stop").hide();
			$("#Button_Analyse").show();

			if ($("#File_Count_Compressible").text() != "0") {
				$("#Button_Compress").show();
			} else {
				$("#Button_Compress").hide();
			}

			if ($("#File_Count_Compressed").text() != "0") {
				$("#Button_Decompress").show();
			} else {
				$("#Button_Decompress").hide();
			}
		},

		reset_folder_summary: function() {
			Gui.set_folder_summary({
				logical_size: 0,
				physical_size: 0,
				compressed: {count: 0, logical_size: 0, physical_size: 0},
				compressible: {count: 0, logical_size: 0, physical_size: 0},
				skipped: {count: 0, logical_size: 0, physical_size: 0}
			});
		},

		set_folder_summary: function(data) {
			$("#Size_Logical").text(Util.bytes_to_human(data.logical_size));
			$("#Size_Physical").text(Util.bytes_to_human(data.physical_size));

			if (data.logical_size > 0) {
				var ratio = (data.physical_size / data.logical_size);
				$("#Compress_Ratio").text(Util.format_number(ratio, 2));
				// $("#Size_Compressed").val(ratio);
			} else {
				$("#Compress_Ratio").text("1.00");
				// $("#Size_Compressed").val(1);
			}

			if (data.logical_size > 0) {
				total = data.logical_size;
				$("#Compressed_Size").text(Util.bytes_to_human(data.compressed.physical_size));
				$("#Compressible_Size").text(Util.bytes_to_human(data.compressible.physical_size));
				$("#Skipped_Size").text(Util.bytes_to_human(data.skipped.physical_size));
				document.getElementById("Breakdown_Compressed").style.width = "" + 100 * (data.compressed.physical_size / total).toFixed(2) + "%";
				document.getElementById("Breakdown_Compressible").style.width = "" + 100 * (data.compressible.physical_size / total).toFixed(2) + "%";
				document.getElementById("Breakdown_Skipped").style.width = "" + 100 * (data.skipped.physical_size / total).toFixed(2) + "%";
			}

			$("#Space_Saved").text(Util.bytes_to_human(data.logical_size - data.physical_size));

			$("#File_Count_Compressed").text(Util.format_number(data.compressed.count, 0));
			$("#File_Count_Compressible").text(Util.format_number(data.compressible.count, 0));
			$("#File_Count_Skipped").text(Util.format_number(data.skipped.count, 0));
		},

		analysis_complete: function() {
			$("#Activity").hide();
			$("#Analysis").show();

		}
	};
})();

$(document).ready(Gui.boot);
