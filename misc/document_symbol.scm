(body) @block
(function_def
	(function_command
		(function)
		(argument_list
			.
			(argument) @identifier
			(argument) *
			)
		)
	) @function

(macro_def
	(macro_command
		(macro)
		(argument_list
			.
			(argument) @identifier
			(argument) *
			)
		)
	) @function

(normal_command
	(identifier) @identifier
	(argument_list
		.
		(argument) @first_arg
		(argument) *
		)
  ) @command
