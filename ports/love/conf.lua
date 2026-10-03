function love.conf(t)
  t.identity = 'chrogue-love'
  t.version = '11.5'
  t.window.title = 'Chrogue'
  t.window.width = 1280
  t.window.height = 720
  t.window.minwidth = 640
  t.window.minheight = 360
  t.window.resizable = true
  t.window.vsync = 1
  -- In a browser, the canvas has the pixels of the screen, also on a screen with a high pixel density.
  t.window.highdpi = love._os == 'Web'
  t.modules.audio = false
  t.modules.joystick = false
  t.modules.physics = false
  t.modules.sound = false
  t.modules.touch = false
  t.modules.video = false
end
